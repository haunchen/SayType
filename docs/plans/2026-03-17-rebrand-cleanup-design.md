# 品牌清理收尾設計

日期：2026-03-17
範圍：清除程式碼中殘留的 Handy/cjpais 引用，完成 SayType 品牌重命名收尾

## 決策紀錄

| 項目 | 決策 |
|------|------|
| About 頁面贊助按鈕 | 改為指向原作者 GitHub（致敬上游） |
| Windows code signing | 移除 signCommand（暫不簽章） |
| vad-rs/rodio 依賴 | 維持現狀 |
| .github/FUNDING.yml | 刪除 |
| blob.handy.computer URL | 程式碼不動，文件加註上游託管說明 |
| scripts/generate-icons.sh | 刪除（icon 已產生） |

## 不動的部分

- `blob.handy.computer` 模型下載 URL（程式碼中的功能性依賴）
- `cjpais/vad-rs`、`cjpais/rodio` Cargo 依賴（之後有需要再 fork）
- `llm_client.rs` User-Agent（已正確包含 SayType + fork 歸屬）
- README 中的 fork 聲明（版權歸屬必須保留）
- CLAUDE.md / AGENTS.md / CRUSH.md 中的模型下載指令

## 變更清單

### A. 元件重命名（4 檔案）

| 檔案 | 變更 |
|------|------|
| `src/components/settings/HandyShortcut.tsx` | 重命名為 `ShortcutRecorder.tsx`，元件名 + interface 同步改 |
| `src/components/settings/index.ts` | 更新 export |
| `src/components/settings/general/GeneralSettings.tsx` | 更新 import + 使用處 |
| `src/components/settings/debug/DebugSettings.tsx` | 更新 import + 使用處 |

### B. About 頁面連結（1 檔案）

| 檔案 | 變更 |
|------|------|
| `src/components/settings/about/AboutSettings.tsx` | source code → `github.com/haunchen/SayType`，donate → `github.com/cjpais/Handy` |

### C. 建置設定（4 檔案）

| 檔案 | 變更 |
|------|------|
| `src-tauri/Cargo.toml` | authors: `["cjpais"]` → `["cjpais", "haunchen"]` |
| `src-tauri/tauri.conf.json` | 移除 windows.signCommand |
| `src/i18n/locales/fr/translation.json` | comment: `cjpais/SayType` → `haunchen/SayType` |
| `src/i18n/locales/vi/translation.json` | comment: `cjpais/SayType` → `haunchen/SayType` |

### D. GitHub 設定（2 檔案）

| 檔案 | 變更 |
|------|------|
| `.github/FUNDING.yml` | 刪除 |
| `.github/ISSUE_TEMPLATE/config.yml` | `cjpais/Handy` → `haunchen/SayType`，「Handy」→「SayType」 |

### E. 文件與腳本（3 檔案）

| 檔案 | 變更 |
|------|------|
| `README.md` | 模型 URL 加註上游託管、移除 `contact@handy.computer`、`handy.computer` 網站改為 `github.com/cjpais/Handy` |
| `CONTRIBUTING.md` | 移除 `contact@handy.computer` |
| `scripts/generate-icons.sh` | 刪除 |
