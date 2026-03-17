# OGG/Opus Audio Decode Implementation Plan

Goal: 在 SayType HTTP API 中實作 OGG/Opus 和 OGG/Vorbis 音訊解碼，讓 Android 端可以傳送 OGG 格式的音訊

Architecture: 使用 symphonia crate（純 Rust）解碼 OGG 容器中的 Opus/Vorbis 音訊，解碼後複用現有的 mono 轉換和 16kHz 重採樣邏輯。只修改 Cargo.toml 和 audio_convert.rs。

Tech Stack: symphonia 0.5（ogg + vorbis + opus features）

---

### Task 1: 新增 symphonia 依賴

Files:
- Modify: `src-tauri/Cargo.toml:76-81,112`

Step 1: 在 optional 依賴區塊新增 symphonia

`src-tauri/Cargo.toml` Line 80（`rand` 行之後）新增：
```toml
symphonia = { version = "0.5", default-features = false, features = ["ogg", "vorbis", "opus"], optional = true }
```

Step 2: 將 symphonia 加入 saytype feature

`src-tauri/Cargo.toml` Line 112：
```toml
# 舊
saytype = ["dep:axum", "dep:tower-http", "dep:base64", "dep:rand"]
# 新
saytype = ["dep:axum", "dep:tower-http", "dep:base64", "dep:rand", "dep:symphonia"]
```

Step 3: 驗證依賴解析
Run: `cd src-tauri && cargo check --features saytype 2>&1 | tail -5`
Expected: 編譯成功（或至少依賴解析無錯誤）

Step 4: Commit
```
feat(saytype): add symphonia dependency for OGG/Opus audio decoding
```

---

### Task 2: 新增 OggDecodeError 並實作 decode_ogg

Files:
- Modify: `src-tauri/src/saytype/audio_convert.rs`

Step 1: 新增 cfg import 和 OggDecodeError variant

在 `audio_convert.rs` 頂部（Line 8 `use std::io::Cursor;` 之後）加上條件 import：
```rust
#[cfg(feature = "saytype")]
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
```

在 `AudioConvertError` enum（Line 12-21）中，在 `UnsupportedSampleFormat` 之前新增：
```rust
    /// OGG 解碼錯誤
    OggDecodeError(String),
```

Step 2: 更新 Display 和 Error impl

在 `fmt::Display` impl（Line 23-34）的 match 中，在 `UnsupportedSampleFormat` arm 之前新增：
```rust
            AudioConvertError::OggDecodeError(e) => write!(f, "OGG decode error: {}", e),
```

`Error::source` impl 不需要改（OggDecodeError 包的是 String，沒有 source）。

Step 3: 替換 convert_from_bytes 中的 ogg placeholder

將 `audio_convert.rs` Line 82-87 的 placeholder：
```rust
        "ogg" => {
            // OGG/Opus 支援將在後續實作
            Err(AudioConvertError::UnsupportedFormat(
                "ogg (not yet implemented)".to_string(),
            ))
        }
```

替換為：
```rust
        "ogg" => decode_ogg(bytes),
```

Step 4: 實作 decode_ogg 函數

在 `decode_wav` 函數之後（Line 135 後）、`resample` 函數之前插入：

```rust
/// 解碼 OGG（Opus/Vorbis）檔案並重採樣至 16kHz mono
fn decode_ogg(bytes: &[u8]) -> Result<AudioConvertResult, AudioConvertError> {
    // 建立 MediaSourceStream
    let cursor = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    // 設定 OGG 格式提示
    let mut hint = Hint::new();
    hint.with_extension("ogg");

    // 探測格式並取得 reader
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AudioConvertError::OggDecodeError(format!("Failed to probe format: {}", e)))?;

    let mut format_reader = probed.format;

    // 取得第一個音訊 track
    let track = format_reader
        .default_track()
        .ok_or_else(|| AudioConvertError::OggDecodeError("No audio track found".to_string()))?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| AudioConvertError::OggDecodeError("Unknown sample rate".to_string()))?;
    let channels = codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1);

    // 建立解碼器
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| AudioConvertError::OggDecodeError(format!("Failed to create decoder: {}", e)))?;

    // 解碼所有 packets
    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format_reader.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break; // 正常結束
            }
            Err(e) => {
                return Err(AudioConvertError::OggDecodeError(format!(
                    "Failed to read packet: {}",
                    e
                )));
            }
        };

        // 跳過非目標 track 的 packet
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(e) => {
                return Err(AudioConvertError::OggDecodeError(format!(
                    "Failed to decode packet: {}",
                    e
                )));
            }
        };

        // 轉換為交錯 f32 samples
        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        if num_frames > 0 {
            let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
            sample_buf.copy_interleaved_ref(decoded);
            all_samples.extend_from_slice(sample_buf.samples());
        }
    }

    if all_samples.is_empty() {
        return Err(AudioConvertError::OggDecodeError(
            "No audio samples decoded".to_string(),
        ));
    }

    // 轉換為 mono（如果是多聲道）
    let mono_samples = if channels > 1 {
        all_samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        all_samples
    };

    // 重採樣至 16kHz（如果需要）
    let target_sample_rate = 16000;
    let final_samples = if sample_rate != target_sample_rate {
        resample(&mono_samples, sample_rate, target_sample_rate)
    } else {
        mono_samples
    };

    let duration_ms = (final_samples.len() as u64 * 1000) / target_sample_rate as u64;

    Ok(AudioConvertResult {
        samples: final_samples,
        duration_ms,
    })
}
```

Step 5: 驗證編譯
Run: `cd src-tauri && cargo check --features saytype 2>&1 | tail -5`
Expected: 編譯成功

Step 6: Commit
```
feat(saytype): implement OGG/Opus and OGG/Vorbis audio decoding
```

---

### Task 3: 更新測試

Files:
- Modify: `src-tauri/src/saytype/audio_convert.rs` (tests module)

Step 1: 更新 test_ogg_not_implemented 為測試損壞資料

將 `audio_convert.rs` 的 `test_ogg_not_implemented` 測試（Line 173-179）替換為：

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
Run: `cd src-tauri && cargo test --features saytype -- audio_convert 2>&1 | tail -20`
Expected: 所有測試通過

Step 3: Commit
```
test(saytype): update OGG decode tests for error cases
```

---

## 執行順序

Task 1 → 2 → 3，依序執行。全部完成後分支上應有 3 個 commit。

## 驗證完成

全部 task 完成後：
```bash
cd src-tauri && cargo test --features saytype -- audio_convert -v
```

Expected: 所有測試通過，包含新的 OGG 錯誤處理測試。
