# PLAN — local-voice-dictation

**Feature:** LOCAL, privacy-preserving voice **input** (dictation) in the chat
composer. User toggles a mic button, speaks, the audio is transcribed by an
**embedded whisper.cpp running on-device**, and the transcript is inserted into
the composer input **for the user to review before sending** (never auto-send).

**Hard constraint:** fully LOCAL — no cloud STT, no browser Web Speech API, no
network dependency at transcription time. Embedded whisper.cpp, fail-soft like
pgvector/biomcp (whisper unavailable → mic self-disables, app still works).

**v1 scope:** dictation-into-composer only. Out of scope (future): real-time
streaming voice-conversation mode, voice output / TTS, barge-in / turn-taking.

---

## Items

- **ITEM-1**: `build_helper/whisper.rs` — build the whisper.cpp **CLI** (`whisper-cli`) from a vendored git submodule via cmake (statically linked, one self-contained executable) for the build's target triple, stage it at `binaries/{target}/whisper/whisper-cli[.exe]`, write a `.version` sidecar, and **fail-soft to a zero-byte stub** on any failure (missing cmake/compiler, unsupported triple, build error). Wire into `build.rs::setup_external_binaries()` with the warn-and-continue `if let Err(e)` block + `cargo:rerun-if-changed=build_helper/whisper.rs`. Add the submodule at `src-app/server/vendor/whisper.cpp` pinned to a fixed tag.
- **ITEM-2**: `modules/voice/embedded.rs` — per-triple `#[cfg] include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/<triple>/whisper/whisper-cli..."))` arms (same 5-triple set as `mcp/utils/embedded.rs`) + `compile_error!` guard for unsupported triples + `whisper_available() -> bool` (`!WHISPER.is_empty()`, stub detection) + `ensure_whisper_extracted() -> Result<&PathBuf>` extracting once (`OnceCell`) to `get_app_data_dir().join("bin")` with `chmod 0o755` on unix.
- **ITEM-3**: `modules/voice/model.rs` — whisper GGML **model manager**: resolve the selected model name → `ggml-<model>.bin`, look for a pre-staged file under `get_app_data_dir().join("voice-models")` (air-gap path), else stream-download it by direct URL from the pinned HF repo (`ggerganov/whisper.cpp`) with a byte-size cap + progress, **sha256-verify against a pinned in-code table of known model hashes**, cache it; expose `model_present(name) -> bool` and `ensure_model(name) -> Result<PathBuf>`. Copies the streaming/cap idiom from `llm_local_runtime/engine/download.rs::download_file`; does **not** reuse the git-LFS/HF-repo `llm_model` fetch path.
- **ITEM-4**: `modules/voice/permissions.rs` — `VoiceTranscribe` (`voice::transcribe`, user-facing) + `VoiceAdminRead` (`voice::admin::read`) + `VoiceAdminManage` (`voice::admin::manage`), mirroring `web_search/permissions.rs`.
- **ITEM-5**: Migrations `00000000000132_create_voice.sql` (singleton `voice_settings` via the `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)` idiom: `enabled`, `model`, `language`, `max_clip_seconds`, `max_upload_bytes`, with range CHECKs + seed `INSERT ... ON CONFLICT DO NOTHING`) and `00000000000133_grant_voice_permissions_to_users.sql` (idempotent `DO $$` grant of `voice::transcribe` to the default `Users` group — copy of migration 98).
- **ITEM-6**: `core/config.rs` — add `#[serde(default)] pub voice: Option<VoiceConfig>` + `struct VoiceConfig { #[serde(default = "default_voice_enabled")] enabled: bool }` (default true), mirroring `WebSearchConfig`. Deploy-level kill switch: read in `voice::init()`; if disabled **or** `!embedded::whisper_available()`, log and return before route/registration side effects.
- **ITEM-7**: `modules/voice/{mod,routes,handlers,models,repository}.rs` — module skeleton mirroring `web_search/`: `#[distributed_slice(MODULE_ENTRIES)]` registration + `AppModule` impl + `init()` (config gate + whisper-availability gate) + `register_routes()`; `models.rs` DTOs deriving `serde` + `JsonSchema`; `repository.rs` singleton `get_settings()` / `update_settings()` (COALESCE patch, `WHERE id = TRUE`).
- **ITEM-8**: `POST /api/voice/transcribe` handler — `RequirePermissions<(VoiceTranscribe,)>`; accept the audio as `multipart` (`field name = "file"`, the `File.upload` pattern); enforce `max_upload_bytes` + `max_clip_seconds` (reject over-cap with a typed error, no silent truncation); sniff/validate the WAV magic bytes; write to a per-request temp file; `ensure_model()` (clear 409-style error if the model isn't present yet); spawn `whisper-cli` (`env_clear` + PATH/HOME/LANG allow-list + `PR_SET_PDEATHSIG` on Linux + `kill_on_drop`) with `-m <model> -f <wav> -l <lang> -nt` emitting JSON/text to stdout; parse the transcript; clean up temp files; return `{ text, language, duration_ms }`.
- **ITEM-9**: Admin surfaces (REST) — `GET /api/voice/settings` (`VoiceAdminRead`) + `PUT /api/voice/settings` (`VoiceAdminManage`, `SyncOrigin`, range-validated, emits `sync_publish(SyncEntity::VoiceSettings, Update, …, Audience::perm::<VoiceAdminRead>())`); `GET /api/voice/model/status` → `{ model, present, size_bytes }`; `POST /api/voice/model/download` (`VoiceAdminManage`) → triggers `ensure_model()` for the configured model and returns status. New sync entity `VoiceSettings`.
- **ITEM-10**: OpenAPI + TS regen (BOTH binaries) — `*_docs(op: TransformOperation)` describers with `with_permission::<Perms>(op)`, `.id("Voice.*")`, `.tag("Voice")`, typed responses; `api_route(...)` registration; run `just openapi-regen` so `ui/openapi.json` + `ui/src/api-client/types.ts` **and** `desktop/ui/*` regenerate (golden `types_ts_parity` stays green). No hand-editing of generated files.
- **ITEM-11**: Desktop native mic permission — add macOS `NSMicrophoneUsageDescription` (+ the mic entitlement) to the Tauri config/`Info.plist` so `getUserMedia` in the webview prompts correctly on macOS; confirm Windows WebView2 surfaces the mic prompt. Decide desktop exposure: the voice chat-extension + settings page **ship on desktop** (server is embedded), so they are NOT added to `CORE_MODULE_BLOCKLIST`.
- **ITEM-12**: Voice chat extension `ui/src/modules/chat/extensions/voice/extension.tsx` + `Voice.store.ts` (`defineExtensionStore`) — recording state machine (`idle | requesting | recording | transcribing | error`), `getUserMedia` + `MediaRecorder` capture, decode + resample to 16 kHz mono PCM via the Web Audio API and encode a WAV `Blob` in-browser (so the server needs no ffmpeg), POST it as `FormData` via `ApiClient.Voice.transcribe(...)`, and on success **append** the transcript with `Stores.Chat.TextStore.getText()/setText()` — never `Stores.Chat.sendMessage()`. Auto-discovered via the `extensions/*/extension.tsx` glob.
- **ITEM-13**: Mic button UI (registered into the `toolbar_actions` slot) — `Mic`/`MicOff` (lucide-react), `variant="ghost"`, the `Tooltip`+`data-tooltip-wrapped` toolbar idiom, a unique `data-testid`; recording indicator (pulsing dot + elapsed timer), cancel-recording affordance (discard), transcribing spinner, disabled state when unsupported / permission-denied / no-model / feature-disabled; a11y (`aria-label`, `aria-pressed` for recording, `aria-live` status region); denied-mic-permission → `message.error(...)` toast.
- **ITEM-14**: Voice admin settings page `ui/src/modules/voice/` (page + store + module registration) — `/settings/voice` ("Voice Dictation") built with `SettingsPageContainer` + the settings-card style, permission-gated in the `settingsAdminPages` slot: enable toggle, model selector (tiny / base / base.en / small), default-language selector, clip/size caps, and a model download + status control; subscribes to `sync:voice_settings` and self-gates the refetch on `VoiceAdminRead`.
- **ITEM-15**: Gallery coverage + state matrix — add gallery cells for the mic button states (idle / recording / transcribing / disabled / error) and the voice settings page (loaded / model-absent) so `check:state-matrix`, `gallery:runtime`, and the Layer A/B visual gates pass for the new surfaces.

## Files to touch

Backend (server):
- `src-app/server/build_helper/whisper.rs` (new)
- `src-app/server/build.rs` (edit — register whisper helper)
- `src-app/server/vendor/whisper.cpp` (new git submodule) + `.gitmodules` (edit)
- `src-app/server/src/modules/voice/mod.rs` (new)
- `src-app/server/src/modules/voice/embedded.rs` (new)
- `src-app/server/src/modules/voice/model.rs` (new)
- `src-app/server/src/modules/voice/permissions.rs` (new)
- `src-app/server/src/modules/voice/routes.rs` (new)
- `src-app/server/src/modules/voice/handlers.rs` (new)
- `src-app/server/src/modules/voice/models.rs` (new)
- `src-app/server/src/modules/voice/repository.rs` (new)
- `src-app/server/src/modules/mod.rs` (edit — declare `voice`)
- `src-app/server/src/core/config.rs` (edit — `VoiceConfig`)
- `src-app/server/src/modules/sync/...` (edit — `VoiceSettings` sync entity)
- `src-app/server/migrations/00000000000132_create_voice.sql` (new)
- `src-app/server/migrations/00000000000133_grant_voice_permissions_to_users.sql` (new)
- `src-app/server/.cargo/config.toml` and/or docs (edit — build-host cmake note)

Desktop (Tauri):
- `src-app/desktop/tauri/tauri.conf.json` / macOS `Info.plist` / entitlements (edit — `NSMicrophoneUsageDescription`)
- `src-app/desktop/tauri/src/modules/backend/mod.rs` (verify — voice enabled on desktop)

Frontend (shared web + desktop via localOverridePlugin fallback):
- `src-app/ui/src/modules/chat/extensions/voice/extension.tsx` (new)
- `src-app/ui/src/modules/chat/extensions/voice/Voice.store.ts` (new)
- `src-app/ui/src/modules/chat/extensions/voice/components/MicButton.tsx` (new)
- `src-app/ui/src/modules/chat/extensions/voice/audio/wav.ts` (new — capture/resample/encode helpers)
- `src-app/ui/src/modules/voice/module.tsx` (new — admin settings module)
- `src-app/ui/src/modules/voice/pages/VoiceSettingsPage.tsx` (new)
- `src-app/ui/src/modules/voice/stores/Voice.store.ts` (new — admin settings store)
- `src-app/ui/src/dev/gallery/...` (edit — gallery entries for new surfaces)
- `src-app/ui/src/api-client/types.ts` + `src-app/ui/openapi.json` (regenerated, not hand-edited)
- `src-app/desktop/ui/src/api-client/types.ts` + `src-app/desktop/ui/openapi.json` (regenerated)

Generated (mechanical — via `just openapi-regen`, excluded from the audit/UI-touch gates):
- `src-app/ui/openapi.json`, `src-app/ui/src/api-client/types.ts`
- `src-app/desktop/ui/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts`

## Patterns to follow

- **Whisper binary build+embed+extract+subprocess** → combine **`build_helper/pgvector.rs`** (build-from-vendored-source via make/cmake, `write_stubs` fail-soft, `OUT_DIR`/`binaries` staging) for the *build* half with **`build_helper/biomcp.rs`** + **`modules/bio_mcp/embedded.rs`** + **`modules/bio_mcp/supervisor.rs`** for the *embed → `include_bytes!` → extract-on-first-use → `env_clear`/`PR_SET_PDEATHSIG` subprocess* half. Keep the 5-triple set identical to **`modules/mcp/utils/embedded.rs`**. Vendored submodule mirrors **`src-app/server/vendor/pgvector`**.
- **Model download / cache / cap / progress** → **`modules/llm_local_runtime/engine/download.rs::download_file`** (streaming + size cap + chunk progress). Cache-dir discipline mirrors **`modules/llm_model/storage.rs`** (`get_app_data_dir()`-rooted). Do **not** reuse the git-LFS/HF-repo `llm_model/handlers/uploads.rs` fetch path (whisper is a direct-URL single file, not GGUF).
- **Module skeleton, permissions, config kill switch, singleton settings, REST GET/PUT, sync emit, OpenAPI `*_docs`** → **`modules/web_search/`** end-to-end (`mod.rs` / `routes.rs` / `handlers.rs` / `models.rs` / `repository.rs` / `permissions.rs`), with the create/grant migration pair mirroring **97/98**.
- **Audio upload handler** → **`modules/file/handlers/upload.rs`** (`axum::extract::Multipart`, `field name = "file"`, magic-byte MIME sniff, per-route `DefaultBodyLimit`, logical size cap constant).
- **Chat composer mic button** → new chat extension registering into the `toolbar_actions` slot like **`modules/chat/extensions/keyboard/extension.tsx`**; icon-button idiom from **`ChatInput.tsx`** (`+` button: `Tooltip` + `data-tooltip-wrapped`); text insertion via **`modules/chat/extensions/text/Text.store.ts`** (`getText`/`setText`). Multipart upload via **`modules/file/stores/File.store.ts:312`** (`FormData` + `ApiClient`).
- **Admin settings page** → **`SettingsPageContainer`** + `McpServerCard`-style cards + `settingsAdminPages` slot + `sync:<entity>` self-gated subscription, mirroring an existing settings module (e.g. **`modules/web-search/`** front-end).
- **Desktop exposure** → config `enabled` gate + (if suppression were wanted) `CORE_MODULE_BLOCKLIST` in `desktop/ui/src/modules/loader.ts`; here voice **ships** on desktop.
