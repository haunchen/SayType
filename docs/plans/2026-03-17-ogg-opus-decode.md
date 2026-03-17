# OGG/Opus Audio Decode Implementation Plan

Goal: 在 SayType HTTP API 中實作 OGG/Opus 和 OGG/Vorbis 音訊解碼

Architecture: 使用 ogg crate 解析容器，偵測 codec 後分流到 lewton（Vorbis）或 opus-decoder（Opus）。解碼後複用現有的 mono 轉換和 16kHz 重採樣邏輯。

Tech Stack: ogg 0.9, lewton 0.10, opus-decoder 0.1（全部純 Rust）

---

### Task 1: 替換依賴為 ogg + lewton + opus-decoder

Files:
- Modify: `src-tauri/Cargo.toml`

已完成。symphonia 已替換為：
```toml
ogg = { version = "0.9", optional = true }
lewton = { version = "0.10", optional = true }
opus-decoder = { version = "0.1", optional = true }
```

saytype feature 已更新為包含這三個依賴。`cargo check --features saytype` 通過。

待 commit。

---

### Task 2: 新增 OggDecodeError 並實作 decode_ogg

Files:
- Modify: `src-tauri/src/saytype/audio_convert.rs`

Step 1: 新增 OggDecodeError variant

在 `AudioConvertError` enum 中，`UnsupportedSampleFormat` 之前新增：
```rust
    /// OGG 解碼錯誤
    OggDecodeError(String),
```

在 `fmt::Display` impl 的 match 中新增：
```rust
            AudioConvertError::OggDecodeError(e) => write!(f, "OGG decode error: {}", e),
```

Step 2: 替換 ogg placeholder 為 decode_ogg 呼叫

將 `convert_from_bytes` 中的 ogg placeholder（Line 82-87）替換為：
```rust
        "ogg" => decode_ogg(bytes),
```

Step 3: 實作 decode_ogg 函數

在 `decode_wav` 函數之後、`resample` 函數之前插入。

decode_ogg 邏輯：
1. 用 `ogg::PacketReader` 讀取第一個 packet
2. 偵測 codec：
   - 前 7 bytes 為 `[0x01, b'v', b'o', b'r', b'b', b'i', b's']` → Vorbis
   - 前 8 bytes 為 `b"OpusHead"` → Opus
   - 否則 → 回傳 OggDecodeError
3. Vorbis 路徑：用 `lewton::inside_ogg::OggStreamReader::new()` 從頭讀取，`read_dec_packet_itl()` 逐 packet 解碼為 i16 interleaved samples，轉 f32
4. Opus 路徑：
   - 解析 OpusHead header（byte 9 = channels, byte 10-11 = pre_skip, byte 12-15 = sample_rate）
   - 建立 `opus_decoder::Decoder::new(sample_rate, channels)`
   - Opus 固定解碼輸出 48kHz
   - 用 `ogg::PacketReader` 逐 packet 讀取，跳過第 2 個 packet（OpusTags）
   - `decoder.decode_float()` 解碼每個 audio packet
5. 轉 mono（與 WAV 相同邏輯）
6. 重採樣至 16kHz（複用 `resample`）

Step 4: 驗證編譯
Run: `cd src-tauri && cargo check --features saytype`
Expected: 編譯成功

Step 5: Commit
```
feat(saytype): implement OGG/Opus and OGG/Vorbis audio decoding
```

---

### Task 3: 更新測試

Files:
- Modify: `src-tauri/src/saytype/audio_convert.rs` (tests module)

Step 1: 替換 test_ogg_not_implemented

將 `test_ogg_not_implemented` 測試替換為：

```rust
    #[test]
    fn test_ogg_invalid_data() {
        let result = convert_from_bytes(&[0, 1, 2, 3], "ogg");
        assert!(matches!(result, Err(AudioConvertError::OggDecodeError(_))));
    }

    #[test]
    fn test_ogg_empty_data() {
        let result = convert_from_bytes(&[], "ogg");
        assert!(matches!(result, Err(AudioConvertError::OggDecodeError(_))));
    }
```

Step 2: 跑測試
Run: `cd src-tauri && cargo test --features saytype -- audio_convert`
Expected: 所有測試通過

Step 3: Commit
```
test(saytype): update OGG decode tests for error cases
```

---

## 執行順序

Task 1 → 2 → 3 依序執行。Task 1 已完成待 commit。

## 驗證完成

```bash
cd src-tauri && cargo test --features saytype -- audio_convert -v
```
