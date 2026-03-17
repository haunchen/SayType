//! 音訊格式轉換
//!
//! 將 Base64 編碼的音訊資料轉換為 16kHz mono f32 samples

use base64::Engine;
use std::error::Error;
use std::fmt;
use std::io::Cursor;

/// 音訊轉換錯誤
#[derive(Debug)]
pub enum AudioConvertError {
    /// Base64 解碼錯誤
    Base64DecodeError(base64::DecodeError),
    /// WAV 解碼錯誤
    WavDecodeError(hound::Error),
    /// OGG 解碼錯誤
    OggDecodeError(String),
    /// 不支援的格式
    UnsupportedFormat(String),
    /// 不支援的取樣格式
    UnsupportedSampleFormat,
}

impl fmt::Display for AudioConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioConvertError::Base64DecodeError(e) => write!(f, "Base64 decode error: {}", e),
            AudioConvertError::WavDecodeError(e) => write!(f, "WAV decode error: {}", e),
            AudioConvertError::OggDecodeError(e) => write!(f, "OGG decode error: {}", e),
            AudioConvertError::UnsupportedFormat(format) => {
                write!(f, "Unsupported format: {}", format)
            }
            AudioConvertError::UnsupportedSampleFormat => write!(f, "Unsupported sample format"),
        }
    }
}

impl Error for AudioConvertError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AudioConvertError::Base64DecodeError(e) => Some(e),
            AudioConvertError::WavDecodeError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<base64::DecodeError> for AudioConvertError {
    fn from(err: base64::DecodeError) -> Self {
        AudioConvertError::Base64DecodeError(err)
    }
}

impl From<hound::Error> for AudioConvertError {
    fn from(err: hound::Error) -> Self {
        AudioConvertError::WavDecodeError(err)
    }
}

/// 音訊轉換結果
pub struct AudioConvertResult {
    /// 16kHz mono f32 samples (-1.0 ~ 1.0)
    pub samples: Vec<f32>,
    /// 音訊長度（毫秒）
    pub duration_ms: u64,
}

/// 從 Base64 字串轉換為 f32 samples
pub fn convert_from_base64(
    base64_data: &str,
    format: &str,
) -> Result<AudioConvertResult, AudioConvertError> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64_data)?;
    convert_from_bytes(&bytes, format)
}

/// 從原始 bytes 轉換為 f32 samples
pub fn convert_from_bytes(
    bytes: &[u8],
    format: &str,
) -> Result<AudioConvertResult, AudioConvertError> {
    match format.to_lowercase().as_str() {
        "wav" => decode_wav(bytes),
        "ogg" => decode_ogg(bytes),
        _ => Err(AudioConvertError::UnsupportedFormat(format.to_string())),
    }
}

/// 解碼 WAV 檔案並重採樣至 16kHz mono
fn decode_wav(bytes: &[u8]) -> Result<AudioConvertResult, AudioConvertError> {
    let cursor = Cursor::new(bytes);
    let mut reader = hound::WavReader::new(cursor)?;
    let spec = reader.spec();

    // 讀取所有 samples 並轉換為 f32
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_value = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_value)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
    };

    // 轉換為 mono（如果是多聲道）
    let mono_samples = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    // 重採樣至 16kHz（如果需要）
    let target_sample_rate = 16000;
    let final_samples = if spec.sample_rate != target_sample_rate {
        resample(&mono_samples, spec.sample_rate, target_sample_rate)
    } else {
        mono_samples
    };

    let duration_ms = (final_samples.len() as u64 * 1000) / target_sample_rate as u64;

    Ok(AudioConvertResult {
        samples: final_samples,
        duration_ms,
    })
}

/// 解碼 OGG 檔案（支援 Vorbis 和 Opus codec）並重採樣至 16kHz mono
fn decode_ogg(bytes: &[u8]) -> Result<AudioConvertResult, AudioConvertError> {
    // 讀取第一個 packet 來偵測 codec
    let cursor = Cursor::new(bytes);
    let mut packet_reader = ogg::PacketReader::new(cursor);
    let first_packet = packet_reader
        .read_packet()
        .map_err(|e| AudioConvertError::OggDecodeError(e.to_string()))?
        .ok_or_else(|| AudioConvertError::OggDecodeError("Empty OGG stream".to_string()))?;

    let data = &first_packet.data;

    // 偵測 codec
    let is_vorbis = data.len() >= 7
        && data[0] == 0x01
        && data[1] == b'v'
        && data[2] == b'o'
        && data[3] == b'r'
        && data[4] == b'b'
        && data[5] == b'i'
        && data[6] == b's';
    let is_opus = data.len() >= 8 && &data[..8] == b"OpusHead";

    if is_vorbis {
        decode_ogg_vorbis(bytes)
    } else if is_opus {
        decode_ogg_opus(bytes, data)
    } else {
        Err(AudioConvertError::OggDecodeError(
            "Unknown OGG codec".to_string(),
        ))
    }
}

/// 解碼 OGG/Vorbis
fn decode_ogg_vorbis(bytes: &[u8]) -> Result<AudioConvertResult, AudioConvertError> {
    let cursor = Cursor::new(bytes);
    let mut reader = lewton::inside_ogg::OggStreamReader::new(cursor)
        .map_err(|e| AudioConvertError::OggDecodeError(e.to_string()))?;

    let channels = reader.ident_hdr.audio_channels as usize;
    let sample_rate = reader.ident_hdr.audio_sample_rate;

    // 逐 packet 解碼為 i16 interleaved samples
    let mut all_samples: Vec<f32> = Vec::new();
    loop {
        match reader.read_dec_packet_itl() {
            Ok(Some(packet)) => {
                // lewton 回傳 i16 interleaved samples
                for s in packet {
                    all_samples.push(s as f32 / 32768.0);
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(AudioConvertError::OggDecodeError(e.to_string()));
            }
        }
    }

    // 轉換為 mono
    let mono_samples = if channels > 1 {
        all_samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        all_samples
    };

    // 重採樣至 16kHz
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

/// 解碼 OGG/Opus
fn decode_ogg_opus(bytes: &[u8], opus_head: &[u8]) -> Result<AudioConvertResult, AudioConvertError> {
    // 解析 OpusHead header
    if opus_head.len() < 19 {
        return Err(AudioConvertError::OggDecodeError(
            "OpusHead header too short".to_string(),
        ));
    }

    let channel_count = opus_head[9] as usize;
    let pre_skip = u16::from_le_bytes([opus_head[10], opus_head[11]]) as usize;
    // bytes 12-15 = input_sample_rate (informational only)
    // Opus 固定解碼輸出 48kHz

    if channel_count == 0 || channel_count > 2 {
        return Err(AudioConvertError::OggDecodeError(
            format!("Unsupported channel count: {}", channel_count),
        ));
    }

    let opus_sample_rate: u32 = 48000;
    let mut decoder = opus_decoder::OpusDecoder::new(opus_sample_rate, channel_count)
        .map_err(|e| AudioConvertError::OggDecodeError(e.to_string()))?;

    // 用新的 PacketReader 從頭重新讀取
    let cursor = Cursor::new(bytes);
    let mut packet_reader = ogg::PacketReader::new(cursor);

    const MAX_FRAME_SIZE: usize = 5760; // 48kHz * 120ms
    let mut all_samples: Vec<f32> = Vec::new();
    let mut packet_index: usize = 0;
    let mut samples_decoded: usize = 0;

    loop {
        let packet = match packet_reader.read_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(e) => {
                return Err(AudioConvertError::OggDecodeError(e.to_string()));
            }
        };

        // 跳過前 2 個 packet（OpusHead + OpusTags）
        if packet_index < 2 {
            packet_index += 1;
            continue;
        }
        packet_index += 1;

        // 解碼 audio packet
        let mut pcm = vec![0.0f32; MAX_FRAME_SIZE * channel_count];
        let samples_per_channel = decoder
            .decode_float(&packet.data, &mut pcm, false)
            .map_err(|e| AudioConvertError::OggDecodeError(e.to_string()))?;

        let total_samples = samples_per_channel * channel_count;

        // 處理 pre_skip：跳過開頭的 pre_skip 個 samples（per channel）
        if samples_decoded + samples_per_channel <= pre_skip {
            // 整個 frame 都在 pre_skip 範圍內，跳過
            samples_decoded += samples_per_channel;
            continue;
        }

        let skip_in_frame = if samples_decoded < pre_skip {
            pre_skip - samples_decoded
        } else {
            0
        };
        samples_decoded += samples_per_channel;

        // 取出需要的 samples（跳過 skip_in_frame 個 per-channel samples）
        let start = skip_in_frame * channel_count;
        all_samples.extend_from_slice(&pcm[start..total_samples]);
    }

    // 轉換為 mono
    let mono_samples = if channel_count > 1 {
        all_samples
            .chunks(channel_count)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        all_samples
    };

    // 重採樣至 16kHz
    let target_sample_rate: u32 = 16000;
    let final_samples = if opus_sample_rate != target_sample_rate {
        resample(&mono_samples, opus_sample_rate, target_sample_rate)
    } else {
        mono_samples
    };

    let duration_ms = (final_samples.len() as u64 * 1000) / target_sample_rate as u64;

    Ok(AudioConvertResult {
        samples: final_samples,
        duration_ms,
    })
}

/// 簡單線性重採樣
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;

    (0..output_len)
        .map(|i| {
            let src_idx = i as f64 * ratio;
            let idx = src_idx.floor() as usize;
            let frac = src_idx.fract() as f32;

            if idx + 1 < samples.len() {
                samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
            } else if idx < samples.len() {
                samples[idx]
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_format() {
        let result = convert_from_bytes(&[], "mp3");
        assert!(matches!(
            result,
            Err(AudioConvertError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn test_ogg_empty_data() {
        let result = convert_from_bytes(&[], "ogg");
        assert!(matches!(
            result,
            Err(AudioConvertError::OggDecodeError(_))
        ));
    }

    #[test]
    fn test_resample() {
        // 48kHz -> 16kHz (3:1 ratio)
        let input: Vec<f32> = (0..48).map(|i| i as f32 / 48.0).collect();
        let output = resample(&input, 48000, 16000);
        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_resample_values() {
        // 簡單測試：確保重採樣後的值在合理範圍內
        let input: Vec<f32> = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let output = resample(&input, 48000, 16000);
        for sample in &output {
            assert!(*sample >= 0.0 && *sample <= 1.0);
        }
    }

    #[test]
    fn test_invalid_wav_data() {
        let result = convert_from_bytes(&[0, 1, 2, 3], "wav");
        assert!(matches!(result, Err(AudioConvertError::WavDecodeError(_))));
    }

    #[test]
    fn test_invalid_base64() {
        let result = convert_from_base64("not-valid-base64!!!", "wav");
        assert!(matches!(
            result,
            Err(AudioConvertError::Base64DecodeError(_))
        ));
    }
}
