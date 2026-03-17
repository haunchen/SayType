# OGG/Opus 音訊解碼設計

日期：2026-03-17
範圍：擴充 SayType HTTP API 的音訊格式支援，實作 OGG/Opus 和 OGG/Vorbis 解碼

## 決策紀錄

| 項目 | 決策 |
|------|------|
| 支援格式 | OGG/Opus + OGG/Vorbis（Android 兩者都可能送） |
| 解碼方案 | ogg + lewton（Vorbis）+ opus-decoder（Opus），全部純 Rust |
| 錯誤處理 | 新增 OggDecodeError(String) variant |

### 方案選擇歷程

1. 最初選擇 symphonia，但實測發現 symphonia 0.5 不含 Opus codec
2. 評估 audiopus（libopus 綁定），但 0.3 只有 rc 版且無 bundled feature
3. 最終選擇：ogg 容器解析 + lewton（Vorbis）+ opus-decoder（純 Rust Opus 解碼）

## 變更範圍

| 檔案 | 變更 |
|------|------|
| `src-tauri/Cargo.toml` | 替換 symphonia → ogg + lewton + opus-decoder（optional，綁到 saytype feature） |
| `src-tauri/src/saytype/audio_convert.rs` | 實作 decode_ogg，新增 OggDecodeError variant |

不動 handlers.rs、types.rs、前端。

## 技術細節

### 依賴

```toml
ogg = { version = "0.9", optional = true }
lewton = { version = "0.10", optional = true }
opus-decoder = { version = "0.1", optional = true }
```

### decode_ogg 函數

流程：
1. 用 `ogg::PacketReader` 讀取第一個 packet
2. 偵測 codec：檢查 packet 前綴 `\x01vorbis`（Vorbis）或 `OpusHead`（Opus）
3. Vorbis 路徑：用 `lewton::inside_ogg::OggStreamReader` 解碼，輸出 i16 interleaved samples
4. Opus 路徑：解析 OpusHead header 取得 channels/sample_rate，用 `opus_decoder::Decoder` 逐 packet 解碼，Opus 固定輸出 48kHz
5. 轉 mono + 重採樣至 16kHz（複用現有函數）

### 錯誤處理

AudioConvertError 新增：
```rust
OggDecodeError(String)  // 錯誤轉為 String，不洩漏內部依賴
```

### 測試

- 更新 test_ogg_not_implemented → 測試損壞 OGG 資料回傳 OggDecodeError
- 新增空資料測試
