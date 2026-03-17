//! SayType API 請求處理器

use crate::managers::transcription::TranscriptionManager;
use crate::saytype::audio_convert::convert_from_base64;
use crate::saytype::types::{
    error_codes, ErrorResponse, StatusResponse, TranscribeRequest, TranscribeResponse,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// 應用程式狀態，包含 Tauri AppHandle
#[derive(Clone)]
pub struct AppState {
    pub app_handle: AppHandle,
    pub token: String,
}

/// 驗證 Authorization header
fn verify_token(
    headers: &HeaderMap,
    expected_token: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or("");

    if token != expected_token {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid token".to_string(),
                code: error_codes::UNAUTHORIZED.to_string(),
            }),
        ));
    }

    Ok(())
}

/// GET /api/status - 取得伺服器狀態
pub async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    // 驗證 token
    if let Err(err) = verify_token(&headers, &state.token) {
        return err.into_response();
    }

    // 取得 TranscriptionManager 狀態
    let (model_loaded, current_model) = {
        if let Some(tm) = state.app_handle.try_state::<Arc<TranscriptionManager>>() {
            (tm.is_model_loaded(), tm.get_current_model())
        } else {
            (false, None)
        }
    };

    let status = if model_loaded { "ready" } else { "loading" };

    let response = StatusResponse {
        status: status.to_string(),
        model_loaded,
        current_model,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// POST /api/transcribe - 執行語音轉文字
pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<TranscribeRequest>,
) -> impl IntoResponse {
    // 驗證 token
    if let Err(err) = verify_token(&headers, &state.token) {
        return err.into_response();
    }

    let start_time = std::time::Instant::now();

    // 取得音訊格式（預設 wav）
    let format = request.format.as_deref().unwrap_or("wav");

    // 轉換音訊
    let audio_result = match convert_from_base64(&request.audio_base64, format) {
        Ok(result) => result,
        Err(e) => {
            let code = match &e {
                crate::saytype::audio_convert::AudioConvertError::Base64DecodeError(_) => {
                    error_codes::DECODE_ERROR
                }
                _ => error_codes::INVALID_FORMAT,
            };
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                    code: code.to_string(),
                }),
            )
                .into_response();
        }
    };

    // 取得 TranscriptionManager 並執行轉錄
    let transcription_result = {
        let tm = match state.app_handle.try_state::<Arc<TranscriptionManager>>() {
            Some(tm) => tm,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "Transcription service not available".to_string(),
                        code: error_codes::MODEL_NOT_LOADED.to_string(),
                    }),
                )
                    .into_response();
            }
        };

        if !tm.is_model_loaded() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Model not loaded".to_string(),
                    code: error_codes::MODEL_NOT_LOADED.to_string(),
                }),
            )
                .into_response();
        }

        tm.transcribe(audio_result.samples)
    };

    match transcription_result {
        Ok(text) => {
            // 潤飾處理（如果請求啟用且設定中有 provider）
            let polished_text = if request.polish.unwrap_or(false) && !text.trim().is_empty() {
                let settings = crate::settings::get_settings(&state.app_handle);
                if settings.post_process_enabled {
                    // 取得 prompt
                    let prompt = settings
                        .post_process_selected_prompt_id
                        .as_ref()
                        .and_then(|id| {
                            settings
                                .post_process_prompts
                                .iter()
                                .find(|p| p.id == *id)
                        })
                        .map(|p| p.prompt.replace("${output}", &text));

                    if let Some(prompt) = prompt {
                        let provider = settings.active_post_process_provider().cloned();
                        match provider {
                            Some(ref p) if p.id == "claude-cli" => {
                                match crate::claude_cli::polish_with_claude_cli(&text, &prompt)
                                    .await
                                {
                                    Ok(polished) => polished,
                                    Err(e) => {
                                        log::warn!("Polish failed: {}", e);
                                        text.clone()
                                    }
                                }
                            }
                            Some(ref p) => {
                                let api_key = settings
                                    .post_process_api_keys
                                    .get(&p.id)
                                    .cloned()
                                    .unwrap_or_default();
                                let model = settings
                                    .post_process_models
                                    .get(&p.id)
                                    .cloned()
                                    .unwrap_or_default();
                                match crate::llm_client::send_chat_completion(
                                    p, api_key, &model, prompt,
                                )
                                .await
                                {
                                    Ok(Some(polished)) => polished,
                                    _ => {
                                        log::warn!("LLM polish failed, returning raw text");
                                        text.clone()
                                    }
                                }
                            }
                            None => text.clone(),
                        }
                    } else {
                        text.clone()
                    }
                } else {
                    text.clone()
                }
            } else {
                text.clone()
            };

            let processing_time_ms = start_time.elapsed().as_millis() as u64;
            let response = TranscribeResponse {
                success: true,
                raw_text: text,
                polished_text,
                language: "auto".to_string(),
                processing_time_ms,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: error_codes::TRANSCRIBE_ERROR.to_string(),
            }),
        )
            .into_response(),
    }
}
