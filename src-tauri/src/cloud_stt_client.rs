use crate::settings::AppSettings;
use log::debug;
use reqwest::StatusCode;
use serde::Deserialize;
use std::fmt;
use std::time::Duration;

#[derive(Debug)]
pub enum CloudSttError {
    Auth(String),
    Network(String),
    Timeout(String),
    Server { status: u16, message: String },
    Parse(String),
}

impl fmt::Display for CloudSttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloudSttError::Auth(message) => {
                write!(f, "Cloud STT authentication error: {}", message)
            }
            CloudSttError::Network(message) => write!(f, "Cloud STT network error: {}", message),
            CloudSttError::Timeout(message) => {
                write!(f, "Cloud STT request timed out: {}", message)
            }
            CloudSttError::Server { status, message } => {
                write!(f, "Cloud STT server error ({}): {}", status, message)
            }
            CloudSttError::Parse(message) => {
                write!(f, "Cloud STT response parse error: {}", message)
            }
        }
    }
}

impl std::error::Error for CloudSttError {}

#[derive(Debug, Deserialize)]
struct CloudTranscriptionResponse {
    text: String,
}

fn trim_error_message(body: &str) -> String {
    const MAX_LEN: usize = 320;
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "No additional details provided by the server".to_string();
    }
    if trimmed.chars().count() <= MAX_LEN {
        return trimmed.to_string();
    }
    let shortened = trimmed.chars().take(MAX_LEN).collect::<String>();
    format!("{}...", shortened)
}

fn normalize_language(language: &str) -> Option<String> {
    let language = language.trim();
    if language.is_empty() || language.eq_ignore_ascii_case("auto") {
        return None;
    }
    if language == "zh-Hans" || language == "zh-Hant" {
        return Some("zh".to_string());
    }
    Some(language.to_string())
}

fn encode_wav(samples: &[f32]) -> Result<Vec<u8>, CloudSttError> {
    // RIFF/WAV header for mono 16-bit PCM at 16kHz.
    let channels: u16 = 1;
    let sample_rate: u32 = 16_000;
    let bits_per_sample: u16 = 16;
    let bytes_per_sample: u16 = bits_per_sample / 8;

    let data_bytes_len = (samples.len() as u32)
        .checked_mul(bytes_per_sample as u32)
        .ok_or_else(|| CloudSttError::Parse("WAV payload too large".to_string()))?;
    let riff_chunk_size = 36u32
        .checked_add(data_bytes_len)
        .ok_or_else(|| CloudSttError::Parse("WAV payload too large".to_string()))?;
    let byte_rate = sample_rate
        .checked_mul(channels as u32)
        .and_then(|v| v.checked_mul(bytes_per_sample as u32))
        .ok_or_else(|| CloudSttError::Parse("WAV payload too large".to_string()))?;
    let block_align = channels
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| CloudSttError::Parse("WAV payload too large".to_string()))?;

    let mut out = Vec::with_capacity((44u32 + data_bytes_len) as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_chunk_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format tag
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes_len.to_le_bytes());

    for sample in samples {
        let pcm_sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&pcm_sample.to_le_bytes());
    }

    Ok(out)
}

fn map_request_error(error: reqwest::Error, timeout_seconds: u32) -> CloudSttError {
    if error.is_timeout() {
        return CloudSttError::Timeout(format!("Request exceeded {} seconds", timeout_seconds));
    }
    CloudSttError::Network(error.to_string())
}

fn classify_status_error(status: StatusCode, body: String) -> CloudSttError {
    let message = trim_error_message(&body);
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => CloudSttError::Auth(message),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
            CloudSttError::Timeout(message)
        }
        _ => CloudSttError::Server {
            status: status.as_u16(),
            message,
        },
    }
}

fn build_client(timeout_seconds: u32) -> Result<reqwest::Client, CloudSttError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds as u64))
        .build()
        .map_err(|e| CloudSttError::Network(format!("Failed to build HTTP client: {}", e)))
}

pub async fn transcribe(
    settings: &AppSettings,
    samples: Vec<f32>,
) -> Result<String, CloudSttError> {
    let provider_id = settings.cloud_stt_provider_id.trim();
    if provider_id.is_empty() {
        return Err(CloudSttError::Parse(
            "Cloud STT provider is not configured".to_string(),
        ));
    }

    let api_key = settings
        .cloud_stt_api_keys
        .get(provider_id)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if api_key.is_empty() {
        return Err(CloudSttError::Auth(format!(
            "Missing API key for provider '{}'",
            provider_id
        )));
    }

    let model = settings
        .cloud_stt_models
        .get(provider_id)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if model.is_empty() {
        return Err(CloudSttError::Parse(format!(
            "Missing cloud STT model for provider '{}'",
            provider_id
        )));
    }

    let base_url = settings
        .cloud_stt_base_url
        .get(provider_id)
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .unwrap_or_default();
    if base_url.is_empty() {
        return Err(CloudSttError::Parse(format!(
            "Missing cloud STT base URL for provider '{}'",
            provider_id
        )));
    }

    let endpoint = if settings.translate_to_english {
        "audio/translations"
    } else {
        "audio/transcriptions"
    };
    let url = format!("{}/{}", base_url, endpoint);
    let timeout_seconds = settings.cloud_stt_request_timeout_seconds;
    let client = build_client(timeout_seconds)?;

    let wav_bytes = encode_wav(&samples)?;
    debug!(
        "Sending cloud STT request to provider '{}' at '{}', samples: {}",
        provider_id,
        url,
        samples.len()
    );

    let file_part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| CloudSttError::Parse(format!("Failed to set WAV MIME type: {}", e)))?;

    let mut form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("model", model)
        .text("response_format", "json");

    if let Some(language) = normalize_language(&settings.selected_language) {
        form = form.text("language", language);
    }

    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| map_request_error(e, timeout_seconds))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(classify_status_error(status, body));
    }

    let payload: CloudTranscriptionResponse = response
        .json()
        .await
        .map_err(|e| CloudSttError::Parse(format!("Invalid JSON body: {}", e)))?;

    if payload.text.trim().is_empty() {
        return Err(CloudSttError::Parse(
            "Response JSON did not include a non-empty 'text' field".to_string(),
        ));
    }

    Ok(payload.text)
}

pub async fn fetch_models(
    base_url: &str,
    api_key: &str,
    timeout_seconds: u32,
) -> Result<Vec<String>, CloudSttError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(CloudSttError::Auth(
            "Missing API key for cloud model listing".to_string(),
        ));
    }

    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(CloudSttError::Parse(
            "Missing cloud STT base URL for model listing".to_string(),
        ));
    }

    let client = build_client(timeout_seconds)?;
    let url = format!("{}/models", base_url);
    debug!("Fetching cloud STT models from '{}'", url);

    let response = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| map_request_error(e, timeout_seconds))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(classify_status_error(status, body));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| CloudSttError::Parse(format!("Invalid models response JSON: {}", e)))?;

    let mut models = Vec::new();
    if let Some(data) = parsed.get("data").and_then(|v| v.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|id| id.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|name| name.as_str()) {
                models.push(name.to_string());
            }
        }
    } else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(id) = entry.as_str() {
                models.push(id.to_string());
            }
        }
    } else {
        return Err(CloudSttError::Parse(
            "Unexpected models response shape".to_string(),
        ));
    }

    Ok(models)
}
