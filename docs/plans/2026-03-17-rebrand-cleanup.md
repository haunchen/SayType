# Rebrand Cleanup Implementation Plan

Goal: 清除程式碼中殘留的 Handy/cjpais 引用，完成 SayType 品牌重命名收尾

Architecture: 純重構任務，不改變功能行為。分 5 個獨立區塊依序執行，每個區塊一個 commit。

Tech Stack: React/TypeScript, Rust/Cargo, YAML, Markdown

---

### Task 1: 元件重命名 HandyShortcut → ShortcutRecorder

Files:
- Rename: `src/components/settings/HandyShortcut.tsx` → `src/components/settings/ShortcutRecorder.tsx`
- Modify: `src/components/settings/index.ts:17`
- Modify: `src/components/settings/general/GeneralSettings.tsx:5,23`
- Modify: `src/components/settings/debug/DebugSettings.tsx:16,49`

Step 1: 重命名檔案並更新元件名稱

`src/components/settings/ShortcutRecorder.tsx`（原 HandyShortcut.tsx）：
- Line 16: `interface HandyShortcutProps {` → `interface ShortcutRecorderProps {`
- Line 23: `export const HandyShortcut: React.FC<HandyShortcutProps> = ({` → `export const ShortcutRecorder: React.FC<ShortcutRecorderProps> = ({`

Step 2: 更新 barrel export

`src/components/settings/index.ts` Line 17:
```ts
// 舊
export { HandyShortcut } from "./HandyShortcut";
// 新
export { ShortcutRecorder } from "./ShortcutRecorder";
```

Step 3: 更新 GeneralSettings import 和使用處

`src/components/settings/general/GeneralSettings.tsx`:
- Line 5: `import { HandyShortcut } from "../HandyShortcut";` → `import { ShortcutRecorder } from "../ShortcutRecorder";`
- Line 23: `<HandyShortcut shortcutId="transcribe" grouped={true} />` → `<ShortcutRecorder shortcutId="transcribe" grouped={true} />`

Step 4: 更新 DebugSettings import 和使用處

`src/components/settings/debug/DebugSettings.tsx`:
- Line 16: `import { HandyShortcut } from "../HandyShortcut";` → `import { ShortcutRecorder } from "../ShortcutRecorder";`
- Line 49-53:
```tsx
// 舊
          <HandyShortcut
            shortcutId="cancel"
            grouped={true}
            disabled={pushToTalk}
          />
// 新
          <ShortcutRecorder
            shortcutId="cancel"
            grouped={true}
            disabled={pushToTalk}
          />
```

Step 5: 驗證
Run: `bun run lint`
Expected: PASS（無 import 錯誤）

Step 6: Commit
```
refactor: rename HandyShortcut to ShortcutRecorder
```

---

### Task 2: About 頁面連結更新

Files:
- Modify: `src/components/settings/about/AboutSettings.tsx:31,60`

Step 1: 更新 source code 連結

`src/components/settings/about/AboutSettings.tsx` Line 60:
```tsx
// 舊
            onClick={() => openUrl("https://github.com/cjpais/Handy")}
// 新
            onClick={() => openUrl("https://github.com/haunchen/SayType")}
```

Step 2: 更新 donate 連結為致敬原作者

`src/components/settings/about/AboutSettings.tsx` Line 31:
```tsx
// 舊
      await openUrl("https://handy.computer/donate");
// 新
      await openUrl("https://github.com/cjpais/Handy");
```

Step 3: Commit
```
fix: update About page links to point to correct repositories
```

---

### Task 3: 建置設定清理

Files:
- Modify: `src-tauri/Cargo.toml:5`
- Modify: `src-tauri/tauri.conf.json:70`
- Modify: `src/i18n/locales/fr/translation.json:2`
- Modify: `src/i18n/locales/vi/translation.json:2`

Step 1: 更新 Cargo.toml authors

`src-tauri/Cargo.toml` Line 5:
```toml
# 舊
authors = ["cjpais"]
# 新
authors = ["cjpais", "haunchen"]
```

Step 2: 移除 Windows signCommand

`src-tauri/tauri.conf.json` Line 69-71:
```json
// 舊
    "windows": {
      "signCommand": "trusted-signing-cli -e https://eus.codesigning.azure.net/ -a CJ-Signing -c cjpais-dev -d SayType %1"
    }
// 新
    "windows": {}
```

Step 3: 更新翻譯檔 comment

`src/i18n/locales/fr/translation.json` Line 2:
```json
// 舊
  "_comment": "French translation for SayType. Contribute at: https://github.com/cjpais/SayType",
// 新
  "_comment": "French translation for SayType. Contribute at: https://github.com/haunchen/SayType",
```

`src/i18n/locales/vi/translation.json` Line 2:
```json
// 舊
  "_comment": "Vietnamese translation for SayType. Contribute at: https://github.com/cjpais/SayType",
// 新
  "_comment": "Vietnamese translation for SayType. Contribute at: https://github.com/haunchen/SayType",
```

Step 4: Commit
```
chore: update build config authors, remove Windows signing, fix translation URLs
```

---

### Task 4: GitHub 設定清理

Files:
- Delete: `.github/FUNDING.yml`
- Modify: `.github/ISSUE_TEMPLATE/config.yml`

Step 1: 刪除 FUNDING.yml

```bash
git rm .github/FUNDING.yml
```

Step 2: 更新 ISSUE_TEMPLATE config.yml

`.github/ISSUE_TEMPLATE/config.yml` 完整替換為：
```yaml
blank_issues_enabled: false
contact_links:
  - name: ✏️ Post-processing / Editing Transcripts
    url: https://github.com/haunchen/SayType/discussions/168
    about: Looking to edit, format, or post-process transcripts? Join this discussion
  - name: ⌨️ Keyboard Shortcuts / Hotkeys
    url: https://github.com/haunchen/SayType/discussions/211
    about: Want different keyboard shortcuts or hotkey configurations? Join this discussion
  - name: 💡 Feature Request or Idea
    url: https://github.com/haunchen/SayType/discussions
    about: Please post feature requests and ideas in our Discussions tab
  - name: 💬 General Discussion
    url: https://github.com/haunchen/SayType/discussions
    about: Ask questions and discuss SayType with the community
```

Step 3: Commit
```
chore: remove FUNDING.yml, update issue template links to haunchen/SayType
```

---

### Task 5: 文件更新與腳本刪除

Files:
- Modify: `README.md:207-215,270,290-291`
- Modify: `CONTRIBUTING.md:308`
- Delete: `scripts/generate-icons.sh`

Step 1: README.md 模型 URL 加註上游託管

在 Line 205（`**Whisper Models (single .bin files):**`）前插入：
```markdown
> **Note:** Model files are hosted by the upstream [Handy](https://github.com/cjpais/Handy) project at `blob.handy.computer`.

```

Step 2: README.md 移除 contact@handy.computer

Line 270:
```markdown
# 舊
5. **Join the discussion** - reach out at [contact@handy.computer](mailto:contact@handy.computer)
# 新
5. **Join the discussion** - open an issue or discussion on [GitHub](https://github.com/haunchen/SayType)
```

Step 3: README.md 更新 Related Projects

Line 290-291:
```markdown
# 舊
- **[Handy CLI](https://github.com/cjpais/handy-cli)** - The original Python command-line version
- **[handy.computer](https://handy.computer)** - Project website with demos and documentation
# 新
- **[Handy](https://github.com/cjpais/Handy)** - The original project that SayType is forked from
- **[Handy CLI](https://github.com/cjpais/handy-cli)** - The original Python command-line version
```

Step 4: CONTRIBUTING.md 移除 contact@handy.computer

Line 308:
```markdown
# 舊
- **Email**: Reach out at [contact@handy.computer](mailto:contact@handy.computer)
# 新
- **GitHub**: Open an issue or discussion on [GitHub](https://github.com/haunchen/SayType)
```

Step 5: 刪除 generate-icons.sh

```bash
git rm scripts/generate-icons.sh
```

如果 `scripts/` 目錄變空，也一併刪除。

Step 6: 驗證
Run: `grep -rn "handy\.computer" README.md CONTRIBUTING.md --include="*.md" | grep -v blob.handy.computer`
Expected: 無輸出（除了 blob.handy.computer 的模型 URL 外不應有 handy.computer 殘留）

Step 7: Commit
```
docs: update documentation links, add upstream hosting note, remove icon generator script
```

---

## 執行順序

Task 1-5 依序執行，每個 task 產出一個 commit。全部完成後分支上應有 5 個 commit。

## 驗證完成

全部 task 完成後執行最終檢查：
```bash
# 確認不該出現的引用已清除
grep -rn "HandyShortcut" src/
grep -rn "handy\.computer" src/ src-tauri/src/ | grep -v blob.handy.computer
grep -rn "cjpais" src/ .github/ | grep -v node_modules | grep -v Cargo.lock
```

Expected: 除了 `Cargo.toml`（vad-rs/rodio 依賴）和 `Cargo.lock` 之外，不應有 cjpais 殘留。
