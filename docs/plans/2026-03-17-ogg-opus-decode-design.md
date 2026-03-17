# OGG/Opus 音訊解碼設計

日期：2026-03-17
範圍：擴充 SayType HTTP API 的音訊格式支援，實作 OGG/Opus 和 OGG/Vorbis 解碼

## 決策紀錄

| 項目 | 決策 |
|------|------|
| 支援格式 | OGG/Opus + OGG/Vorbis（Android 兩者都可能送） |
| 解碼方案 | symphonia（純 Rust，一個 crate 支援兩種 codec） |
| 錯誤處理 | 新增 OggDecodeError(String) variant |

## 變更範圍

| 檔案 | 變更 |
|------|------|
| `src-tauri/Cargo.toml` | 新增 symphonia 依賴（optional，綁到 saytype feature） |
| `src-tauri/src/saytype/audio_convert.rs` | 實作 decode_ogg，新增 OggDecodeError variant |

不動 handlers.rs、types.rs、前端。

## 技術細節

### symphonia 依賴

```toml
symphonia = { version = "0.5", features = ["ogg", "vorbis", "opus"], optional = true }
```

saytype feature 加入 `"dep:symphonia"`。

### decode_ogg 函數

流程與 decode_wav 對稱：
1. MediaSourceStream + FormatReader 讀取 OGG 容器
2. 自動偵測 codec（Opus 或 Vorbis）
3. 讀取所有 audio packets 解碼為 f32 samples
4. 轉 mono（複用與 WAV 相同的混合邏輯）
5. 重採樣至 16kHz（複用現有 resample 函數）

### 錯誤處理

AudioConvertError 新增：
```rust
OggDecodeError(String)  // symphonia 錯誤轉為 String，不洩漏內部依賴
```

### 測試

- 更新 test_ogg_not_implemented → 測試損壞 OGG 資料回傳 OggDecodeError
- format 分派測試確認 "ogg" 走 decode_ogg 路徑
