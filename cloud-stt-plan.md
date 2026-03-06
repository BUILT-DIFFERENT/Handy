# Add Groq Cloud STT to Handy With Optional Warm Local Fallback

## Summary
Implement a new cloud transcription path using Groq Audio API (`/openai/v1/audio/transcriptions` and `/openai/v1/audio/translations`) while preserving Handy's current fast capture flow.
Recording remains local-first and immediate; after stop, audio is uploaded to cloud for transcription.
If cloud fails/times out, fallback to local model transcription (per chosen mode), with a new toggle controlling whether the local model is preloaded in background while cloud mode is active.

Defaults chosen:
1. `Cloud with fallback`
2. `3 min hard cap`
3. `Background local preload while in cloud mode = off`

## Scope
1. Add cloud STT provider support focused on Groq.
2. Add settings/UI for cloud mode, API key, model, timeout behavior, fallback, and warm-local toggle.
3. Enforce max upload duration at 180s (3 min) with deterministic handling.
4. Keep existing local model path intact and backward compatible.
5. Keep post-processing pipeline behavior unchanged (still optional and separate).

Out of scope:
1. Streaming/partial cloud transcript rendering.
2. Multi-provider cloud STT in this first pass (design remains extensible).

## Public API / Types / Interface Changes

### Rust settings (`AppSettings`)
Add:
1. `transcription_backend: TranscriptionBackend`
   Values: `local | groq_cloud`
2. `cloud_stt_enabled: bool` (optional if backend enum is sufficient; prefer enum only)
3. `cloud_stt_provider_id: String` default `"groq"`
4. `cloud_stt_api_keys: HashMap<String, String>` (initial key for `groq`)
5. `cloud_stt_models: HashMap<String, String>` default `groq -> whisper-large-v3-turbo`
6. `cloud_stt_base_url: HashMap<String, String>` default `groq -> https://api.groq.com/openai/v1` (kept configurable for enterprise/proxy)
7. `cloud_stt_fallback_to_local: bool` default `true`
8. `cloud_stt_preload_local_model: bool` default `false`
9. `cloud_stt_max_audio_seconds: u32` default `180` (validated max 300)
10. `cloud_stt_request_timeout_seconds: u32` default `90` (bounded range, e.g. 15..300)

### New/updated Tauri commands
Add:
1. `change_transcription_backend_setting(backend: String)`
2. `change_cloud_stt_api_key_setting(provider_id: String, api_key: String)`
3. `change_cloud_stt_model_setting(provider_id: String, model: String)`
4. `change_cloud_stt_base_url_setting(provider_id: String, base_url: String)`
5. `change_cloud_stt_fallback_setting(enabled: bool)`
6. `change_cloud_stt_preload_local_model_setting(enabled: bool)`
7. `change_cloud_stt_max_audio_seconds_setting(seconds: u32)`
8. `change_cloud_stt_request_timeout_setting(seconds: u32)`
9. `fetch_cloud_stt_models(provider_id: String) -> Vec<String>`

Update:
1. Specta exports (`src/bindings.ts`) to include new commands and settings fields.
2. Frontend `Settings` typing and store update map.

## Backend Implementation Plan

### 1) Add cloud transcription client
Create `src-tauri/src/cloud_stt_client.rs`:
1. Build provider-aware headers (Bearer token, content-type multipart).
2. Convert in-memory samples (`Vec<f32>`, 16k mono) to WAV bytes in-memory.
3. Submit multipart request:
   1. `/audio/transcriptions` for normal transcription
   2. `/audio/translations` when `translate_to_english = true`
4. Include optional fields:
   1. `language` when selected language != `auto` (normalize `zh-Hans`/`zh-Hant` to `zh`)
   2. `response_format=json`
   3. `model`
5. Parse response into typed struct with `text`.
6. Return normalized errors (HTTP status, timeout, parse, auth).

### 2) Add routing layer in transcription pipeline
Introduce a new routing function in `actions.rs` (or a new `transcription_router.rs`):
1. On stop, keep current flow: get samples quickly from recorder.
2. Enforce max duration:
   1. Compute `max_samples = cloud_stt_max_audio_seconds * 16000`.
   2. If exceeded, trim to first `max_samples`.
   3. Log truncation with exact seconds.
3. Branch by backend:
   1. `local`: existing `TranscriptionManager::transcribe`.
   2. `groq_cloud`: call cloud client.
4. If cloud fails and fallback enabled:
   1. Ensure local model is loaded (load lazily if needed).
   2. Run local transcription.
5. Preserve downstream behavior (Chinese variant conversion, optional post-process, history, paste).

### 3) Respect warm-local toggle without changing capture responsiveness
Update `TranscribeAction::start` preloading behavior:
1. Current always-calls `tm.initiate_model_load()`.
2. Change to:
   1. If backend `local`: always preload.
   2. If backend `groq_cloud` and `cloud_stt_preload_local_model=true`: preload.
   3. Else: skip preload.
This keeps start/capture path fast and avoids unnecessary memory usage in cloud mode.

### 4) Settings defaults + migration
Update `settings.rs`:
1. Add defaults for new cloud fields.
2. Add migration helper similar to `ensure_post_process_defaults`.
3. Guarantee existing users auto-receive valid cloud settings without breaking old settings stores.

### 5) Command registration
Update `lib.rs` command collection and frontend bindings generation to include all new cloud commands.

## Frontend Implementation Plan

### 1) Settings UI
Add a dedicated section under model/general settings:
1. `Transcription backend`: `Local` or `Groq Cloud`.
2. Cloud-only controls:
   1. API key input
   2. Model select + refresh models
   3. Optional base URL field (if editable)
   4. Fallback to local toggle
   5. Preload local model in background toggle
   6. Max audio duration selector (fixed to 3 min in UI now, advanced could expose 5)
   7. Request timeout selector

### 2) Store integration
Update `settingsStore.ts`:
1. Extend `settingUpdaters` with new cloud settings commands.
2. Add `fetchCloudSttModels` helper + model options cache (same pattern as post-process models).
3. Keep optimistic updates and rollback behavior.

### 3) i18n
1. Add new `en` translation keys.
2. Propagate keys to all locales to satisfy `scripts/check-translations.ts`.

## Performance and Reliability Details
1. Capture performance is preserved by keeping recording logic untouched and non-blocking.
2. Cloud upload/transcription runs in async task after stop; no additional delay at capture start.
3. 3-minute hard cap limits payload and tail latency.
4. Retry policy: no automatic retry by default (avoid duplicate charges/latency); rely on one-shot + local fallback.
5. Timeout and error classes logged distinctly (`auth`, `network`, `timeout`, `server`, `parse`).

## Testing Plan

### Rust unit tests
1. Settings migration:
   1. Old settings load and new cloud fields are auto-filled.
2. Audio limit:
   1. <=180s unchanged.
   2. >180s trimmed exactly to cap.
3. Router behavior:
   1. Local backend uses local path only.
   2. Cloud success returns cloud text.
   3. Cloud failure + fallback enabled uses local text.
   4. Cloud failure + fallback disabled returns error.
4. Header/request builder:
   1. Correct endpoint for transcribe vs translate.
   2. Correct auth header and form fields.

### Integration tests (mocked HTTP)
1. Mock Groq success response returns expected text.
2. Mock 401/429/500 and timeout paths map to expected fallback/error behavior.
3. Ensure history/paste still receive final text path consistently.

### Frontend tests
1. Settings UI toggles visibility by backend selection.
2. API key/model changes call correct commands.
3. New settings persist across refresh and round-trip through `get_app_settings`.

### Manual acceptance scenarios
1. Cloud mode, valid key/model, short recording -> fast cloud transcript pasted.
2. Cloud mode with network failure + fallback on + local model installed -> local transcript pasted.
3. Cloud mode with fallback off + failure -> clear error and no paste.
4. Cloud mode + preload off: no local model load event until fallback is needed.
5. Long dictation >180s -> truncation occurs, result still returned.

## Rollout / Compatibility
1. Backward compatible settings migration, default backend remains `local`.
2. Feature ships behind explicit user selection (`Groq Cloud`).
3. Existing local model and onboarding flow remain unchanged.
4. No breaking changes to existing hotkeys or post-processing behavior.

## Assumptions and Defaults
1. "Groq models" means Groq cloud speech-to-text API from `api-reference.md`.
2. Initial cloud provider scope is Groq only.
3. Default cloud model: `whisper-large-v3`.
4. Duration cap: `180s` hard trim (upper technical max remains 300s in validation).
5. Fallback to local: enabled by default.
6. Background local preload in cloud mode: disabled by default.
