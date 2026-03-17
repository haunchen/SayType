# LLM Polish Integration Implementation Plan

Goal: 整合 LLM 潤飾功能，透過 Claude Code CLI 或現有 API provider 對語音轉錄結果進行文字修正

Architecture: 新增 `claude_cli.rs` 模組封裝 CLI 呼叫，將 `claude-cli` 加入現有的 post-processing provider 體系。桌面端已有完整的 provider 分派、prompt 管理、歷史紀錄基礎設施，只需在 `actions.rs` 加一個分派 branch。HTTP API 端在 `handlers.rs` 接上同樣的潤飾邏輯。

Tech Stack: tokio::process::Command, serde_json（已有依賴）

---

### Task 1: 建立 claude_cli.rs 模組

Files:
- Create: `src-tauri/src/claude_cli.rs`
- Modify: `src-tauri/src/lib.rs:10`（加 mod 宣告）

Step 1: 建立 claude_cli.rs

`src-tauri/src/claude_cli.rs` 完整內容：

```rust
//! Claude Code CLI 整合
//!
//! 透過 Claude Code CLI 的非互動模式執行文字潤飾

use log::{debug, error};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Claude CLI 呼叫的超時時間
const CLI_TIMEOUT: Duration = Duration::from_secs(30);

/// 透過 Claude Code CLI 潤飾文字
///
/// `transcript` - 要潤飾的原始文字
/// `prompt` - 潤飾指令（已替換好 ${output} 變數）
pub async fn polish_with_claude_cli(transcript: &str, prompt: &str) -> Result<String, String> {
    if transcript.trim().is_empty() {
        return Err("Empty transcript".to_string());
    }

    debug!("Starting Claude CLI polish, prompt length: {} chars", prompt.len());

    let mut child = Command::new("claude")
        .args(["-p", prompt, "--output-format", "json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Claude Code not installed. Install from https://claude.ai/code".to_string()
            } else {
                format!("Failed to start Claude CLI: {}", e)
            }
        })?;

    // 寫入 transcript 到 stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(transcript.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to Claude CLI stdin: {}", e))?;
        // drop stdin to close pipe
    }

    // 等待完成（帶超時）
    let output = timeout(CLI_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "Claude CLI timed out (30s)".to_string())?
        .map_err(|e| format!("Claude CLI process error: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not authenticated") || stderr.contains("auth") {
            return Err("Claude Code not authenticated. Run 'claude auth login' first.".to_string());
        }
        return Err(format!(
            "Claude CLI failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    // 解析 JSON 回應
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 from Claude CLI: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Claude CLI JSON output: {}", e))?;

    let result = json
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Claude CLI JSON missing 'result' field".to_string())?;

    if result.trim().is_empty() {
        return Err("Claude CLI returned empty result".to_string());
    }

    if let Some(cost) = json.get("cost").and_then(|v| v.as_f64()) {
        debug!("Claude CLI polish cost: ${:.4}", cost);
    }

    debug!("Claude CLI polish succeeded, output length: {} chars", result.len());
    Ok(result.to_string())
}
```

Step 2: 在 lib.rs 加入 mod 宣告

`src-tauri/src/lib.rs` Line 10（`mod llm_client;` 之後）新增：
```rust
mod claude_cli;
```

Step 3: 驗證編譯
Run: `cd src-tauri && cargo check`
Expected: 編譯成功

Step 4: Commit
```
feat: add claude_cli module for Claude Code CLI integration
```

---

### Task 2: 新增 claude-cli provider 到設定

Files:
- Modify: `src-tauri/src/settings.rs`

Step 1: 在 default_post_process_providers() 中新增 claude-cli

在 `src-tauri/src/settings.rs` 的 `default_post_process_providers()` 函數中，在 Apple Intelligence 區塊之前（即 `vec![...]` 的最後一個元素後，`#[cfg(all(target_os = "macos"...` 之前）新增：

```rust
        PostProcessProvider {
            id: "claude-cli".to_string(),
            label: "Claude Code (Local)".to_string(),
            base_url: String::new(),
            allow_base_url_edit: false,
            models_endpoint: None,
        },
```

Step 2: 驗證編譯
Run: `cd src-tauri && cargo check`
Expected: 編譯成功

Step 3: Commit
```
feat: add claude-cli as post-processing provider option
```

---

### Task 3: 在桌面端 post-processing 分派中加入 claude-cli

Files:
- Modify: `src-tauri/src/actions.rs:30-155`

Step 1: 在 maybe_post_process_transcription 中加入 claude-cli 分派

在 `src-tauri/src/actions.rs` 的 `maybe_post_process_transcription` 函數中，Apple Intelligence 區塊結束後（Line 131 的 `}` 之後）、取得 api_key 之前（Line 133 的 `let api_key = ...` 之前），插入 claude-cli 分派：

```rust
    // Claude Code CLI provider
    if provider.id == "claude-cli" {
        return match crate::claude_cli::polish_with_claude_cli(transcription, &processed_prompt)
            .await
        {
            Ok(result) => {
                debug!(
                    "Claude CLI post-processing succeeded. Output length: {} chars",
                    result.len()
                );
                Some(result)
            }
            Err(err) => {
                error!("Claude CLI post-processing failed: {}", err);
                None
            }
        };
    }
```

注意：claude-cli 不需要 model 和 api_key 檢查，所以要放在那些檢查之前。但目前 model 為空的檢查在 Line 52-58 會提前 return None。需要修改 model 檢查，讓 claude-cli 跳過：

將 Line 52-58 的 model 檢查：
```rust
    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return None;
    }
```

改為：
```rust
    if model.trim().is_empty() && provider.id != "claude-cli" {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return None;
    }
```

Step 2: 驗證編譯
Run: `cd src-tauri && cargo check`
Expected: 編譯成功

Step 3: Commit
```
feat: integrate claude-cli provider in desktop post-processing
```

---

### Task 4: 在 HTTP API handler 中接上潤飾

Files:
- Modify: `src-tauri/src/saytype/handlers.rs:144-154`

Step 1: 在 transcribe handler 中加入潤飾邏輯

在 `src-tauri/src/saytype/handlers.rs` 的 `transcribe` 函數中，將 Line 144-154 的成功分支：

```rust
    match transcription_result {
        Ok(text) => {
            let processing_time_ms = start_time.elapsed().as_millis() as u64;
            let response = TranscribeResponse {
                success: true,
                raw_text: text.clone(),
                polished_text: text, // 目前不處理潤飾
                language: "auto".to_string(),
                processing_time_ms,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
```

替換為：

```rust
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
```

Step 2: 驗證編譯
Run: `cd src-tauri && cargo check --features saytype`
Expected: 編譯成功

Step 3: Commit
```
feat(saytype): integrate LLM polish in HTTP API transcribe handler
```

---

## 執行順序

Task 1 → 2 → 3 → 4 依序執行。全部完成後分支上應有 4 個 commit。

## 驗證完成

```bash
cd src-tauri && cargo check --features saytype
```

手動驗證（需 Claude Code 已安裝且登入）：
1. 啟動 app，在 post-processing 設定中選擇 "Claude Code (Local)" provider
2. 啟用 post-processing，錄一段語音，確認潤飾功能運作
3. 透過 HTTP API 發送 `polish: true` 的請求，確認回應中 `polished_text` 不同於 `raw_text`
