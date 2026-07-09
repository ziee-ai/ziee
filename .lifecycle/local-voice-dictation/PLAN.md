# PLAN — local-voice-dictation (full managed whisper runtime)

**Feature:** LOCAL, privacy-preserving voice **input** (dictation) in the chat
composer, backed by a **full managed whisper.cpp speech-to-text runtime** that
mirrors `llm_local_runtime` (version registry + download + update + admin UI +
settings + health/idle-reap lifecycle). User toggles a mic button, speaks, the
audio is transcribed **on-device** by a managed `whisper-server` instance, and
the transcript is inserted into the composer input **for review before sending**
(never auto-send).

**Hard constraint:** fully LOCAL — no cloud STT, no browser Web Speech API, no
network at transcription time. Fail-soft like pgvector/biomcp: whisper
unavailable → mic self-disables, app still works.

**Delivery model (decided, DEC-1):** whisper.cpp ships **no** official
Linux/macOS per-triple binaries, and a `build.rs`-time source build can only
produce the *host* triple. So we follow the **exact llm_local_runtime model**: a
ziee-owned **`ziee-ai/whisper.cpp` fork** whose CI builds `whisper-server` from
source per `{platform}-{arch}-{backend}`, publishes GitHub Releases, and the
ziee server **downloads on demand at runtime** with a version registry, update
flow, and admin UI. Nothing whisper-sized is embedded in the ziee binary.

**v1 scope:** dictation-into-composer + the full runtime management surface.
Out of scope (future): streaming voice-conversation mode, TTS/voice output,
barge-in, GPU backends beyond CPU (the backend axis is wired but only `cpu`
ships v1), per-recording language override, global hotkey.

---

## Items

### A. Engine delivery — fork + download + version registry (mirror `llm_local_runtime/engine` + `runtime_version`)

- **ITEM-1**: `ziee-ai/whisper.cpp` fork + release CI (separate repo, like `ziee-ai/llama.cpp`) — a GitHub Actions matrix that builds `whisper-server` from source via CMake per `{platform}-{arch}-{backend}` (v1: `cpu`; cuda/metal/vulkan slots reserved), packages `whisper-server[-.exe]` as `whisper-server-{platform}-{arch}-{backend}.{tar.gz|zip}`, and publishes each with a `.sha256` sidecar (cosign `.sig` slot reserved), matching the `ziee-ai/llama.cpp` asset-naming scheme (`{stem}-{platform}-{arch}-{backend}.{ext}`).
- **ITEM-2**: `modules/voice/engine/download.rs` — download-on-demand from `engine_repo() = "ziee-ai/whisper.cpp"`: `get_latest_version` + `list_releases` (GitHub releases API, retry/backoff), `archive_name` builder + `asset_backend`/`available_backends`, streaming `download_file` with the 2 GiB cap + progress callback + sha256 verify, safe `extract_tar_gz`/`extract_zip`, cache under `get_app_data_dir()/whisper-runtime/binaries/{version}/{platform}-{arch}-{backend}/`. Debug-only mirror seams `WHISPER_RUNTIME_RELEASE_MIRROR` / `WHISPER_RUNTIME_API_MIRROR` (compiled out of release).
- **ITEM-3**: `voice_runtime_versions` table + `modules/voice/runtime_version/{models,repository}.rs` — installed-version registry (`version, platform, arch, backend, binary_path, is_system_default`, `UNIQUE(version,platform,arch,backend)`, partial index on default). Repo: `create`, `list_all`, `get_by_id`, `get_latest_version`, `get_system_default`, `clear_system_default`, `set_system_default`, `delete`, `usage` (in-use check).
- **ITEM-4**: `modules/voice/runtime_version/download_task.rs` — detached download task: `DashMap<key, Arc<DownloadTask>>` keyed `whisper@{version}@{backend}`, `broadcast` SSE channel (`Connected/Progress/Complete/Failed` via `sse_event_enum!`), `start_or_join` dedupe + terminal-entry replace-on-retry, race against a `SHUTDOWN` notify, `sync_publish(VoiceRuntimeVersion, Create)` on success. Survives page reload via list-active + re-subscribe.
- **ITEM-5**: `modules/voice/binary_manager.rs` — `select_version` (system-default → latest by `created_at`), `check_for_updates` (list_releases diff installed + host `{platform,arch}` via `gpu_detect::host_platform/arch`, per-release `binary_ready`/`available_backends`/`recommended_backend`/`size_bytes`), `set_system_default` (clear-then-set), `sync_cache` (disk scan → back-fill DB rows).

### B. Whisper model management (direct-URL ggml files)

- **ITEM-6**: `modules/voice/model/{mod,download}.rs` + `voice_models`-free design — resolve selected model name → `ggml-<model>.bin` direct HF URL; detect a pre-staged file under `get_app_data_dir()/voice-models/` (air-gap); else stream-download (reusing the `download_file` cap/progress + a `download_task`-style SSE channel), **sha256-verify against a pinned in-code known-hash table**, cache; `model_present(name)` / `ensure_model(name) -> PathBuf`. No git-LFS/HF-repo machinery (unlike `llm_model`).

### C. Managed whisper-server instance lifecycle (mirror `deployment` + `auto_start` + `reaper` + `engine/health`)

- **ITEM-7**: `modules/voice/deployment/local.rs` — spawn ONE managed `whisper-server --host 127.0.0.1 --port <ephemeral> -m <model.bin> [-l <lang>]` as a hardened subprocess (`env_clear` + PATH/HOME/LANG allow-list + `PR_SET_PDEATHSIG` on Linux + `kill_on_drop`), post-spawn loopback-bind verify, `/` health GET, log capture + `subscribe_logs`. `DeploymentManager` `OnceCell` singleton owns it.
- **ITEM-8**: `modules/voice/engine/health.rs` — health state machine (`Starting/Healthy/Unhealthy/Crashed/Restarting/Failed/Stopped`, `ExponentialBackoff` 1→60 s, `SlidingWindow` flap cap 5/60 s → `Failed`, `from_persisted`) adapted from `llm_local_runtime/engine/health.rs`.
- **ITEM-9**: `modules/voice/auto_start.rs` — lazy single-flight `ensure_running` of the singleton whisper-server loaded with the configured model (one `OnceCell` guard, `HEALTH` map, `MAX_RESTART_ATTEMPTS=5`); model change ⇒ drain + restart (or `whisper-server` hot-swap); crash ⇒ backoff-restart honoring the state machine; `ensure_restored` from the persisted instance row; `clear_failed`.
- **ITEM-10**: `modules/voice/reaper.rs` — 60 s tick (debug `WHISPER_RUNTIME_REAPER_TICK_MS`): flush `last_used_at`, `monitor_health` (probe `/`, feed the state machine, persist `state`), and if `idle_unload_secs>0` drain+stop the instance when idle. `drain_and_stop` waits up to `drain_timeout_secs`.
- **ITEM-11**: `voice_runtime_instance` table (singleton-ish: `local_port, base_url, status, state, state_changed_at, restart_attempts, last_failure_reason, last_used_at, active_model`) + `voice_runtime_settings` singleton (`id BOOLEAN PK CHECK(id=TRUE)`; `idle_unload_secs` default 1800, `auto_start_timeout_secs` 30, `drain_timeout_secs` 30, `model` default `base`, `language` default `auto`, `max_clip_seconds` 120, `max_upload_bytes` 33554432, `enabled` true — **no `allow_unsigned_downloads`**, dropped upstream) + migration.

### D. Transcription endpoint + config + permissions + module wiring

- **ITEM-12**: `modules/voice/permissions.rs` — `VoiceTranscribe` (`voice::transcribe`, user), `VoiceAdminRead` (`voice::admin::read`), `VoiceAdminManage` (`voice::admin::manage`). (Cleaner web_search-style read/manage split rather than the llm runtime's 9-perm split — DEC.)
- **ITEM-13**: Migrations `00000000000132_create_voice.sql` (the three tables above + seeds) and `00000000000133_grant_voice_permissions_to_users.sql` (idempotent `array_append` of `voice::transcribe` to the default `Users` group; admin covered by the `*` wildcard).
- **ITEM-14**: `core/config.rs` — `#[serde(default)] pub voice: Option<VoiceConfig>` + `VoiceConfig { enabled: bool = true }` deploy kill switch; read in `voice::init()` (also gate on binary/model availability).
- **ITEM-15**: `POST /api/voice/transcribe` handler — `RequirePermissions<(VoiceTranscribe,)>`; multipart WAV (`field "file"`), per-route `DefaultBodyLimit::max(max_upload_bytes ceiling)`; enforce `max_upload_bytes` + `max_clip_seconds` (reject over-cap, no truncation); WAV magic-sniff; `auto_start::ensure_running` (returns a clear "model/binary not ready" error if the instance can't come up); forward the WAV to the loopback whisper-server `/inference` (`response_format=json`, `language`), parse the transcript; return `{ text, language, duration_ms }`; `proxy::touch_last_used`.
- **ITEM-16**: `modules/voice/{mod,routes}.rs` — `#[distributed_slice(MODULE_ENTRIES)]` registration (order near 32), `AppModule` + `init()` (config + availability gate; spawn `reaper`), `register_routes()` merging `voice_router()`; declare `pub mod voice;` in `modules/mod.rs`.

### E. Admin REST surface (versions / update / settings / model / instance)

- **ITEM-17**: Version + update REST (`voice/runtime_version/handlers.rs`) — `GET /api/voice/versions` (AdminRead), `GET /api/voice/versions/check-updates` (AdminRead), `POST /api/voice/versions/download` (AdminManage) + `GET /api/voice/versions/downloads` + `GET /api/voice/versions/downloads/{key}/events` (SSE), `DELETE /api/voice/versions/{id}` (AdminManage, **in-use guard** → 409 if the active instance/default references it, optional `?remove_binary`), `POST /api/voice/versions/{id}/set-default` (AdminManage, emits `sync:voice_runtime_version`), `POST /api/voice/versions/sync-cache` (AdminManage).
- **ITEM-18**: Settings + model + instance REST — `GET/PUT /api/voice/settings` (AdminRead/AdminManage, range-validated, emits `sync:voice_settings`); `GET /api/voice/model/status` + `POST /api/voice/model/download` (+ SSE events) (AdminManage); `GET /api/voice/instance` + `POST /api/voice/instance/restart|stop` + `GET /api/voice/instance/logs/stream` (SSE) (AdminRead/AdminManage). `base_url` redacted for non-admins.

### F. OpenAPI + desktop

- **ITEM-19**: OpenAPI + TS regen (BOTH binaries) — `*_docs(op)` describers with `with_permission::<Perms>`, `.tag("Voice")`, typed responses; `api_route` registration; new sync entities `VoiceSettings` + `VoiceRuntimeVersion`; run `just openapi-regen` so `ui/` + `desktop/ui/` `openapi.json` + `api-client/types.ts` regenerate (golden `types_ts_parity` green). No hand-edits.
- **ITEM-20**: Desktop native mic permission — macOS `NSMicrophoneUsageDescription` (+ mic entitlement) in the Tauri config so `getUserMedia` prompts; verify Windows WebView2 prompt on the Windows build host. Voice extension + admin page **ship on desktop** (server embedded) — NOT in `CORE_MODULE_BLOCKLIST`.

### G. Frontend

- **ITEM-21**: Voice chat extension `ui/src/modules/chat/extensions/voice/{extension.tsx,Voice.store.ts,components/MicButton.tsx,audio/wav.ts}` — `defineExtensionStore` recording state machine (`idle|requesting|recording|transcribing|error`), `getUserMedia`+`MediaRecorder` capture, decode+resample to 16 kHz mono + WAV encode in-browser (server stays ffmpeg-free), POST via `ApiClient.Voice.transcribe(FormData)`, **append** transcript via `Stores.Chat.TextStore.getText()/setText()` (never `sendMessage`). `Mic`/`MicOff` button into the `toolbar_actions` slot with a unique `data-testid`; recording indicator (pulsing dot + elapsed timer), cancel/discard, transcribing spinner, disabled state (unsupported / permission-denied / feature-off / no-model), a11y (`aria-label`, `aria-pressed`, `aria-live`), denied-permission `message.error` toast.
- **ITEM-22**: Voice admin module `ui/src/modules/voice/` (mirror `modules/llm-local-runtime/`) — `module.tsx` registers `/settings/voice` ("Voice Dictation") in `settingsAdminPages` (gated `anyOf:[VoiceAdminRead]`); page stacks `InstalledVersionsCard` (list + set-default + delete-with-confirm) + `AvailableVersionsCard` (check-updates + install with SSE `<Progress>`) + `VoiceConfigCard` (idle/timeout + model selector + language + caps + enable toggle) + a model download/status card; stores mirror the runtime set (`VoiceRuntimeVersion`, `VoiceRuntimeUpdate`, `VoiceDownloadProgress` SSE consumer with reload-safe re-subscribe, `VoiceConfig`, `VoiceModel`); each subscribes to its `sync:<entity>` and self-gates the refetch on `VoiceAdminRead` (no-403 rule).
- **ITEM-23**: Gallery coverage + state matrix — gallery cells for the mic button (idle/recording/transcribing/disabled/error) and the voice admin page (versions installed/available/downloading/empty, model absent) so `check:state-matrix`, `gallery:runtime`, and Layer A/B gates pass.

## Files to touch

Backend (server):
- `src-app/server/src/modules/voice/mod.rs`, `routes.rs`, `permissions.rs`, `binary_manager.rs`, `reaper.rs`, `auto_start.rs`, `repository.rs`, `events.rs` (new)
- `src-app/server/src/modules/voice/engine/{mod,download,health}.rs` (new)
- `src-app/server/src/modules/voice/runtime_version/{mod,models,repository,handlers,download_task}.rs` (new)
- `src-app/server/src/modules/voice/runtime_settings/{mod,models,handlers}.rs` (new)
- `src-app/server/src/modules/voice/model/{mod,download}.rs` (new)
- `src-app/server/src/modules/voice/deployment/{mod,local,manager}.rs` (new)
- `src-app/server/src/modules/voice/handlers.rs` (new — transcribe + instance)
- `src-app/server/src/modules/voice/models.rs` (new — DTOs)
- `src-app/server/src/modules/mod.rs` (edit — `pub mod voice;`)
- `src-app/server/src/core/config.rs` (edit — `VoiceConfig`)
- `src-app/server/src/modules/sync/...` (edit — `VoiceSettings` + `VoiceRuntimeVersion` entities)
- `src-app/server/migrations/00000000000132_create_voice.sql` (new)
- `src-app/server/migrations/00000000000133_grant_voice_permissions_to_users.sql` (new)
- `src-app/server/tests/voice/*.rs` + `src-app/server/tests/voice/mock_release.rs` + `src-app/stub-whisper-server/` (new test fixtures; `stub-engine` is the template)
- **External repo (not this monorepo):** `ziee-ai/whisper.cpp` fork + `.github/workflows/release.yml`

Desktop (Tauri):
- `src-app/desktop/tauri/tauri.conf.json` / macOS `Info.plist` / entitlements (edit — `NSMicrophoneUsageDescription`)
- `src-app/desktop/tauri/src/modules/backend/mod.rs` (verify — voice enabled on desktop)

Frontend (shared web + desktop via localOverridePlugin fallback):
- `src-app/ui/src/modules/chat/extensions/voice/{extension.tsx,Voice.store.ts,components/MicButton.tsx,audio/wav.ts}` (new)
- `src-app/ui/src/modules/voice/{module.tsx,pages/VoiceSettingsPage.tsx,components/*,stores/*,events/*}` (new)
- `src-app/ui/src/dev/gallery/...` (edit — gallery entries)
- `src-app/ui/openapi.json` + `src-app/ui/src/api-client/types.ts` (regenerated)
- `src-app/desktop/ui/openapi.json` + `src-app/desktop/ui/src/api-client/types.ts` (regenerated)

Generated (mechanical, via `just openapi-regen`; excluded from audit/UI-touch gates):
- `**/openapi.json`, `**/api-client/types.ts` (both workspaces)

## Patterns to follow

- **Whole runtime module** → mirror **`modules/llm_local_runtime/`** end-to-end (module layout, `OnceCell` `DeploymentManager`, `MODULE_ENTRIES` order ~32, `init()` spawning the reaper). Copy-adapt (not extend-`EngineType`) so whisper stays cleanly separate from the LLM engines.
- **Binary download + version registry + update flow** → **`llm_local_runtime/engine/download.rs`** (repo slug, `archive_name`, `get_latest_version`/`list_releases`, 2 GiB cap, mirror seams), **`runtime_version/{repository,handlers,download_task}.rs`** (SSE download task, in-use delete guard, set-default), **`binary_manager.rs`** (`select_version`, `check_for_updates`, `sync_cache`). Fork/release mechanics mirror **`ziee-ai/llama.cpp`**'s CI + asset naming.
- **Health / auto-start / idle-reap / drain** → **`engine/health.rs`**, **`auto_start.rs`**, **`reaper.rs`**, **`deployment/local.rs`** (subprocess hardening also cross-checked against **`bio_mcp/supervisor.rs`**).
- **Model download (direct URL)** → **`engine/download.rs::download_file`** (streaming/cap/progress) + **`llm_model/storage.rs`** cache-dir discipline; **NOT** the git-LFS `llm_model/handlers/uploads.rs` path.
- **Runtime settings singleton + REST GET/PUT + sync** → **`runtime_settings/*`** (drop `allow_unsigned_downloads`); permission-gated handlers + `sync_publish` per **`web_search/handlers.rs`**.
- **Config kill switch + create/grant migration pair** → **`web_search`** `Option<Config>{enabled}` + migrations **97/98** (singleton `id BOOLEAN PK CHECK(id=TRUE)`).
- **Transcribe upload handler** → **`file/handlers/upload.rs`** (`Multipart`, magic-sniff, per-route `DefaultBodyLimit`, logical cap).
- **Admin UI** → mirror **`modules/llm-local-runtime/`** (7 stores incl. the SSE `RuntimeDownloadProgress` reload-safe consumer, `InstalledVersionsCard`/`AvailableVersionsCard`/`RuntimeConfigCard`, `settingsAdminPages` slot, `sync:<entity>` self-gated refetch).
- **Mic button + composer insertion** → new chat extension into `toolbar_actions` (like `extensions/keyboard/extension.tsx`); text via `extensions/text/Text.store.ts`; multipart upload via `file/stores/File.store.ts` FormData idiom.
- **Test fixtures** → `stub-whisper-server` workspace member modeled on **`stub-engine/`**; `MockReleaseServer` modeled on **`tests/llm_local_runtime/mock_release.rs`**; debug mirror-env seams for deterministic download/update tests.
