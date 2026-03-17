//! Claude Code CLI 整合
//!
//! 透過 Claude Code CLI 的非互動模式執行文字潤飾

use log::debug;
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
