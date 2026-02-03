use crate::settings::TranscriptionProvider;
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, REFERER, USER_AGENT};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::io::Cursor;

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

fn build_headers(provider: &TranscriptionProvider, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    headers.insert(
        REFERER,
        HeaderValue::from_static("https://github.com/cjpais/Handy"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Handy/1.0 (+https://github.com/cjpais/Handy)"),
    );
    headers.insert("X-Title", HeaderValue::from_static("Handy"));

    if !api_key.is_empty() {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| format!("Invalid authorization header value: {}", e))?,
        );
    }

    Ok(headers)
}

fn create_client(
    provider: &TranscriptionProvider,
    api_key: &str,
) -> Result<reqwest::Client, String> {
    let headers = build_headers(provider, api_key)?;
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn encode_wav_bytes(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buffer = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut buffer, spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV data: {}", e))?;

    Ok(buffer.into_inner())
}

pub async fn transcribe_audio(
    provider: &TranscriptionProvider,
    api_key: String,
    model: &str,
    samples: &[f32],
    language: Option<String>,
    prompt: Option<String>,
    translate_to_english: bool,
) -> Result<String, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let endpoint = if translate_to_english {
        "audio/translations"
    } else {
        "audio/transcriptions"
    };
    let url = format!("{}/{}", base_url, endpoint);

    debug!("Sending cloud transcription request to: {}", url);

    let client = create_client(provider, &api_key)?;
    let wav_bytes = encode_wav_bytes(samples)?;

    let file_part = Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to build multipart file: {}", e))?;

    let mut form = Form::new().part("file", file_part).text("model", model.to_string());

    if let Some(language) = language {
        if !language.trim().is_empty() && !translate_to_english {
            form = form.text("language", language);
        }
    }

    if let Some(prompt) = prompt {
        if !prompt.trim().is_empty() {
            form = form.text("prompt", prompt);
        }
    }

    form = form.text("response_format", "json".to_string());

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        return Err(format!(
            "Transcription request failed with status {}: {}",
            status, error_text
        ));
    }

    let parsed: TranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse transcription response: {}", e))?;

    Ok(parsed.text)
}

pub async fn fetch_models(
    provider: &TranscriptionProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    debug!("Fetching transcription models from: {}", url);

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();

    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    } else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}
