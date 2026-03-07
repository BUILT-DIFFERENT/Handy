use crate::settings::{
    cloud_provider_for_backend, AppSettings, TranscriptionBackend, CLOUD_STT_DEEPGRAM_PROVIDER_ID,
};
use futures_util::{SinkExt, StreamExt};
use log::{debug, warn};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::async_runtime::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;
use url::Url;

#[derive(Debug, Clone)]
pub enum DeepgramStreamingError {
    Config(String),
    Connect(String),
    Auth(String),
    Network(String),
    Server(String),
    FinalizeTimeout(String),
    EmptyTranscript,
    SessionClosed,
}

impl fmt::Display for DeepgramStreamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeepgramStreamingError::Config(message) => write!(f, "Deepgram config error: {}", message),
            DeepgramStreamingError::Connect(message) => {
                write!(f, "Deepgram connection error: {}", message)
            }
            DeepgramStreamingError::Auth(message) => {
                write!(f, "Deepgram authentication error: {}", message)
            }
            DeepgramStreamingError::Network(message) => write!(f, "Deepgram network error: {}", message),
            DeepgramStreamingError::Server(message) => write!(f, "Deepgram server error: {}", message),
            DeepgramStreamingError::FinalizeTimeout(message) => {
                write!(f, "Deepgram finalize timeout: {}", message)
            }
            DeepgramStreamingError::EmptyTranscript => write!(f, "Deepgram returned an empty transcript"),
            DeepgramStreamingError::SessionClosed => write!(f, "Deepgram session is not active"),
        }
    }
}

impl std::error::Error for DeepgramStreamingError {}

enum SessionCommand {
    Audio(Vec<f32>),
    Finalize(oneshot::Sender<Result<String, DeepgramStreamingError>>),
    Cancel,
}

#[derive(Default)]
struct TranscriptAccumulator {
    segments: BTreeMap<i64, String>,
}

impl TranscriptAccumulator {
    fn add_result(&mut self, result: &DeepgramResultMessage) {
        if !result.is_final.unwrap_or(false) {
            return;
        }

        let transcript = result
            .channel
            .alternatives
            .first()
            .map(|alt| alt.transcript.trim())
            .unwrap_or("");
        if transcript.is_empty() {
            return;
        }

        let key = (result.start * 1000.0).round() as i64;
        self.segments.insert(key, transcript.to_string());
    }

    fn final_text(&self) -> String {
        self.segments
            .values()
            .filter(|text| !text.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }
}

#[derive(Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

#[derive(Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Deserialize)]
struct DeepgramResultMessage {
    start: f64,
    #[serde(default)]
    is_final: Option<bool>,
    #[serde(default)]
    speech_final: Option<bool>,
    #[serde(default)]
    from_finalize: Option<bool>,
    channel: DeepgramChannel,
}

#[derive(Deserialize)]
struct DeepgramErrorMessage {
    #[serde(default)]
    err_code: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Clone)]
struct SessionHandle {
    tx: mpsc::UnboundedSender<SessionCommand>,
    fingerprint: String,
}

#[derive(Clone, Default)]
pub struct DeepgramStreamingManager {
    current_session: Arc<Mutex<Option<SessionHandle>>>,
}

impl DeepgramStreamingManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_preconnected(
        &self,
        settings: AppSettings,
    ) -> Result<(), DeepgramStreamingError> {
        if settings.transcription_backend != TranscriptionBackend::DeepgramStreaming {
            self.cancel_session();
            return Ok(());
        }
        if settings.translate_to_english {
            return Err(DeepgramStreamingError::Config(
                "Deepgram streaming does not support translate-to-English mode.".to_string(),
            ));
        }
        let fingerprint = session_fingerprint(&settings);
        if let Some(session) = self.current_session.lock().unwrap().clone() {
            if session.fingerprint == fingerprint {
                debug!("Deepgram preconnect cache hit");
                return Ok(());
            }
        }
        self.cancel_session();

        let provider_id = cloud_provider_for_backend(settings.transcription_backend)
            .unwrap_or(CLOUD_STT_DEEPGRAM_PROVIDER_ID);
        let api_key = settings
            .cloud_stt_api_keys
            .get(provider_id)
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if api_key.is_empty() {
            return Err(DeepgramStreamingError::Auth(
                "Missing API key for Deepgram streaming.".to_string(),
            ));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        *self.current_session.lock().unwrap() = Some(SessionHandle { tx, fingerprint });
        spawn(run_session(settings, api_key, rx));
        Ok(())
    }

    pub fn push_audio_frame(&self, frame: Vec<f32>) {
        let maybe_session = self.current_session.lock().unwrap().clone();
        if let Some(session) = maybe_session {
            let _ = session.tx.send(SessionCommand::Audio(frame));
        }
    }

    pub async fn finalize_and_collect(
        &self,
        finalize_timeout_seconds: u32,
    ) -> Result<String, DeepgramStreamingError> {
        let session = self
            .current_session
            .lock()
            .unwrap()
            .take()
            .ok_or(DeepgramStreamingError::SessionClosed)?;

        let (tx, rx) = oneshot::channel();
        session
            .tx
            .send(SessionCommand::Finalize(tx))
            .map_err(|_| DeepgramStreamingError::SessionClosed)?;

        timeout(Duration::from_secs(finalize_timeout_seconds as u64), rx)
            .await
            .map_err(|_| {
                DeepgramStreamingError::FinalizeTimeout(format!(
                    "No final transcript received within {} seconds",
                    finalize_timeout_seconds
                ))
            })?
            .map_err(|_| DeepgramStreamingError::SessionClosed)?
    }

    pub fn cancel_session(&self) {
        if let Some(session) = self.current_session.lock().unwrap().take() {
            let _ = session.tx.send(SessionCommand::Cancel);
        }
    }
}

async fn run_session(
    settings: AppSettings,
    api_key: String,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
) {
    if let Err(err) = run_session_inner(settings, api_key, &mut cmd_rx).await {
        warn!("Deepgram streaming session ended with error: {}", err);

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                SessionCommand::Finalize(reply_tx) => {
                    let _ = reply_tx.send(Err(err.clone()));
                    break;
                }
                SessionCommand::Cancel => break,
                SessionCommand::Audio(_) => {}
            }
        }
    }
}

async fn run_session_inner(
    settings: AppSettings,
    api_key: String,
    cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
) -> Result<(), DeepgramStreamingError> {
    let request = build_request(&settings, &api_key)?;
    let connect_start = Instant::now();
    let (stream, _) = connect_async(request)
        .await
        .map_err(classify_connect_error)?;
    debug!(
        "Deepgram WebSocket connected in {:?}",
        connect_start.elapsed()
    );

    let (mut sink, mut stream_rx) = stream.split();
    let mut transcript = TranscriptAccumulator::default();
    let mut finalize_tx: Option<oneshot::Sender<Result<String, DeepgramStreamingError>>> = None;
    let mut total_samples_sent = 0usize;
    let max_samples = settings.cloud_stt_max_audio_seconds as usize * 16_000;

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    SessionCommand::Audio(frame) => {
                        if total_samples_sent >= max_samples {
                            continue;
                        }

                        let remaining = max_samples.saturating_sub(total_samples_sent);
                        let frame = if frame.len() > remaining {
                            frame[..remaining].to_vec()
                        } else {
                            frame
                        };
                        total_samples_sent += frame.len();

                        let pcm_bytes = encode_pcm16(&frame);
                        sink.send(Message::Binary(pcm_bytes.into()))
                            .await
                            .map_err(|err| DeepgramStreamingError::Network(err.to_string()))?;
                    }
                    SessionCommand::Finalize(reply_tx) => {
                        sink.send(Message::Text("{\"type\":\"Finalize\"}".to_string().into()))
                            .await
                            .map_err(|err| DeepgramStreamingError::Network(err.to_string()))?;
                        finalize_tx = Some(reply_tx);
                    }
                    SessionCommand::Cancel => {
                        let _ = sink.send(Message::Close(None)).await;
                        return Ok(());
                    }
                }
            }
            Some(message) = stream_rx.next() => {
                match message {
                    Ok(Message::Text(text)) => {
                        if try_handle_server_error(&text, &mut finalize_tx) {
                            return Err(DeepgramStreamingError::Server(text.to_string()));
                        }

                        if let Ok(result) = serde_json::from_str::<DeepgramResultMessage>(&text) {
                            transcript.add_result(&result);
                            // Return as soon as Deepgram marks the utterance as final, instead of
                            // waiting only for an explicit from_finalize event.
                            if result.from_finalize.unwrap_or(false)
                                || result.speech_final.unwrap_or(false)
                            {
                                let final_text = transcript.final_text();
                                if let Some(reply_tx) = finalize_tx.take() {
                                    let result = if final_text.is_empty() {
                                        Err(DeepgramStreamingError::EmptyTranscript)
                                    } else {
                                        Ok(final_text)
                                    };
                                    let _ = reply_tx.send(result);
                                }

                                let _ = sink.send(Message::Text("{\"type\":\"CloseStream\"}".to_string().into())).await;
                                let _ = sink.send(Message::Close(None)).await;
                                return Ok(());
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        if let Some(reply_tx) = finalize_tx.take() {
                            let final_text = transcript.final_text();
                            let result = if final_text.is_empty() {
                                Err(DeepgramStreamingError::SessionClosed)
                            } else {
                                Ok(final_text)
                            };
                            let _ = reply_tx.send(result);
                        }
                        return Ok(());
                    }
                    Ok(Message::Ping(payload)) => {
                        sink.send(Message::Pong(payload))
                            .await
                            .map_err(|err| DeepgramStreamingError::Network(err.to_string()))?;
                    }
                    Ok(Message::Binary(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                    Err(err) => {
                        return Err(DeepgramStreamingError::Network(err.to_string()));
                    }
                }
            }
            else => {
                break;
            }
        }
    }

    if let Some(reply_tx) = finalize_tx.take() {
        let final_text = transcript.final_text();
        let result = if final_text.is_empty() {
            Err(DeepgramStreamingError::SessionClosed)
        } else {
            Ok(final_text)
        };
        let _ = reply_tx.send(result);
    }

    debug!("Deepgram streaming session completed");
    Ok(())
}

fn build_request(
    settings: &AppSettings,
    api_key: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, DeepgramStreamingError> {
    let provider_id = cloud_provider_for_backend(settings.transcription_backend)
        .unwrap_or(CLOUD_STT_DEEPGRAM_PROVIDER_ID);
    let base_url = settings
        .cloud_stt_base_url
        .get(provider_id)
        .map(|value| value.trim())
        .unwrap_or("");
    if base_url.is_empty() {
        return Err(DeepgramStreamingError::Config(
            "Missing Deepgram WebSocket URL.".to_string(),
        ));
    }

    let model = settings
        .cloud_stt_models
        .get(provider_id)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("nova-3");

    let mut url = Url::parse(base_url)
        .map_err(|err| DeepgramStreamingError::Config(format!("Invalid Deepgram URL: {}", err)))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("model", model);
        query.append_pair("encoding", "linear16");
        query.append_pair("sample_rate", "16000");
        query.append_pair("channels", "1");
        query.append_pair("interim_results", "true");
        query.append_pair("vad_events", "true");
        query.append_pair("smart_format", "true");
        query.append_pair("punctuate", "true");
        if let Some(language) = normalize_language(&settings.selected_language) {
            query.append_pair("language", &language);
        }
    }

    let mut request = url
        .to_string()
        .into_client_request()
        .map_err(|err| DeepgramStreamingError::Config(err.to_string()))?;
    request.headers_mut().insert(
        "Authorization",
        format!("Token {}", api_key)
            .parse()
            .map_err(|err| DeepgramStreamingError::Config(format!("Invalid auth header: {}", err)))?,
    );
    Ok(request)
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

fn session_fingerprint(settings: &AppSettings) -> String {
    let provider_id = cloud_provider_for_backend(settings.transcription_backend)
        .unwrap_or(CLOUD_STT_DEEPGRAM_PROVIDER_ID);
    format!(
        "{}|{}|{}|{}",
        provider_id,
        settings
            .cloud_stt_models
            .get(provider_id)
            .map(String::as_str)
            .unwrap_or_default(),
        settings
            .cloud_stt_base_url
            .get(provider_id)
            .map(String::as_str)
            .unwrap_or_default(),
        normalize_language(&settings.selected_language).unwrap_or_else(|| "auto".to_string())
    )
}

fn encode_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn classify_connect_error(
    error: tokio_tungstenite::tungstenite::Error,
) -> DeepgramStreamingError {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            let status = response.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                DeepgramStreamingError::Auth(format!("HTTP {}", status))
            } else {
                DeepgramStreamingError::Server(format!("HTTP {}", status))
            }
        }
        other => DeepgramStreamingError::Connect(other.to_string()),
    }
}

fn try_handle_server_error(
    text: &str,
    finalize_tx: &mut Option<oneshot::Sender<Result<String, DeepgramStreamingError>>>,
) -> bool {
    match serde_json::from_str::<DeepgramErrorMessage>(text) {
        Ok(error_message) if error_message.description.is_some() || error_message.err_code.is_some() => {
            if let Some(reply_tx) = finalize_tx.take() {
                let _ = reply_tx.send(Err(DeepgramStreamingError::Server(
                    error_message
                        .description
                        .unwrap_or_else(|| "Unknown Deepgram server error".to_string()),
                )));
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_accumulator_ignores_interim_results() {
        let mut accumulator = TranscriptAccumulator::default();
        accumulator.add_result(&DeepgramResultMessage {
            start: 0.0,
            is_final: Some(false),
            speech_final: Some(false),
            from_finalize: Some(false),
            channel: DeepgramChannel {
                alternatives: vec![DeepgramAlternative {
                    transcript: "hello".to_string(),
                }],
            },
        });
        assert!(accumulator.final_text().is_empty());
    }

    #[test]
    fn transcript_accumulator_orders_and_replaces_final_segments() {
        let mut accumulator = TranscriptAccumulator::default();
        accumulator.add_result(&DeepgramResultMessage {
            start: 1.0,
            is_final: Some(true),
            speech_final: Some(false),
            from_finalize: Some(false),
            channel: DeepgramChannel {
                alternatives: vec![DeepgramAlternative {
                    transcript: "world".to_string(),
                }],
            },
        });
        accumulator.add_result(&DeepgramResultMessage {
            start: 0.0,
            is_final: Some(true),
            speech_final: Some(false),
            from_finalize: Some(false),
            channel: DeepgramChannel {
                alternatives: vec![DeepgramAlternative {
                    transcript: "hello".to_string(),
                }],
            },
        });
        accumulator.add_result(&DeepgramResultMessage {
            start: 1.0,
            is_final: Some(true),
            speech_final: Some(false),
            from_finalize: Some(false),
            channel: DeepgramChannel {
                alternatives: vec![DeepgramAlternative {
                    transcript: "world!".to_string(),
                }],
            },
        });

        assert_eq!(accumulator.final_text(), "hello world!");
    }
}
