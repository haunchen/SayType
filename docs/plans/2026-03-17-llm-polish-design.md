# LLM 潤飾整合設計

日期：2026-03-17
範圍：整合 LLM 潤飾功能，支援 Claude Code CLI 和現有 API provider

## 決策紀錄

| 項目 | 決策 |
|------|------|
| Claude Agent SDK | 不適用（Python/TS SDK，SayType 後端是 Rust） |
| 方案 | 透過 Claude Code CLI 非互動模式呼叫 |
| 與現有 API 的關係 | 並存，claude-cli 作為新 provider 選項 |
| 觸發場景 | HTTP API + 桌面端，兩者可各自開關 |
| Prompt | 可自訂，帶預設 prompt |
| 目標使用者 | 開發者本人（可假設 Claude Code 已安裝） |

## 變更範圍

| 區塊 | 檔案 | 說明 |
|------|------|------|
| A. CLI 呼叫 | 新增 `src-tauri/src/claude_cli.rs` | 封裝 Claude Code CLI 呼叫 |
| B. HTTP API | 修改 `src-tauri/src/saytype/handlers.rs` | polish 參數處理 |
| C. 桌面端 | 修改 `src-tauri/src/managers/transcription.rs` | 轉錄後可選潤飾 |
| D. 設定 | 修改 settings 相關檔案 | provider 選擇 + 自訂 prompt |

不動 `llm_client.rs`（現有 API provider 方案保留）。

## 技術細節

### claude_cli.rs

單一公開函數：
```rust
pub async fn polish_with_claude_cli(
    transcript: &str,
    prompt: &str,
) -> Result<String, String>
```

- 用 `tokio::process::Command` 非同步呼叫 `claude -p <prompt> --output-format json`
- transcript 透過 stdin 送入
- 解析 JSON 回應的 `result` 欄位
- 30 秒 timeout

### 預設 prompt

```
修正語音轉文字的錯誤、移除贅詞和重複，保持原意和語氣不變。只輸出修正後的文字，不要加任何說明。
```

### Provider 設定

新增 `claude-cli` provider：
- id: "claude-cli"
- name: "Claude Code (Local)"
- 不需要 API key、base_url、model
- 前端選擇此 provider 時隱藏 API key / base_url / model 輸入框

### 潤飾流程

HTTP API：`request.polish == Some(true)` → 依 provider 分派 → claude-cli 或 llm_client
桌面端：設定中「自動潤飾」開啟 → 同樣分派邏輯 → 結果取代 raw text 貼入

### 錯誤處理

| 情況 | 處理 |
|------|------|
| CLI 不存在 | 回傳 "Claude Code not installed" |
| CLI 未登入 | 偵測 stderr，回傳 "Claude Code not authenticated" |
| 超時 | 30 秒 timeout |
| 潤飾失敗 | 不阻塞，回傳 raw_text + error |
| 空轉錄結果 | 跳過潤飾 |

潤飾失敗不阻塞主流程，使用者永遠能拿到 raw_text。
