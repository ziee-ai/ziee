# PLAN — voice-model-mgmt

Bring **whisper-model management** (download + upload + a library with active-model
selection) to the voice-dictation module, mirroring (a) the voice module's existing
engine-*binary* download-with-progress pattern (`runtime_version/download_task.rs` +
`AvailableVersionsCard`/`InstalledVersionsCard`/`VoiceDownloadProgress.store`) and
(b) the LLM-provider model upload flow (`AddLocalLlmModelUploadDrawer` +
`LlmModelUpload.store`).

## Current state (surveyed) — what exists vs. the gap

- Whisper models today = a hardcoded 4-name allow-list (`tiny/base/base.en/small`,
  `model.rs:64`) with pinned sha256, fetched by direct HF URL, stored at
  `<app_data>/voice-models/ggml-<name>.bin`. Model = the `voice_runtime_settings.model`
  string. **No `voice_models` table.**
- `POST /voice/model/download` is **synchronous** (logging-only progress cb,
  `instance_handlers.rs:183`); `GET /voice/model/status` reports only the one configured
  model. **No upload, no model list, no delete, no per-model select.**
- The engine-*binary* download already has the full async SSE pattern (DashMap task
  registry + `broadcast` + `/events` SSE) in `runtime_version/download_task.rs` — the
  exact thing to mirror for model download.
- Permissions: `voice::transcribe`, `voice::admin::read`, `voice::admin::manage`
  (`permissions.rs`); model management already rides on `admin::{read,manage}`.

## Approved product decisions (from user, plan-time)
1. Download source = curated **sha256-pinned catalog** + **arbitrary HF-repo/URL**
   (non-catalog → SSRF-validated, sha256 computed & surfaced as **unverified**; catalog
   stays fail-closed pinned).
2. **Multi-model library** with **Set-active + Delete** (active drives
   `voice_runtime_settings.model`).
3. Catalog breadth = standard + turbo + `.en` + **quantized** (`q5_1`/`q8_0`).

## Items

### Backend — data + catalog
- **ITEM-1**: Add `voice_models` table (migration `00000000000155_create_voice_models.sql`):
  installed-model rows `(id uuid pk, name, filename unique, source enum['catalog','url','upload'], source_url nullable, size_bytes bigint, sha256 char(64) nullable, verified bool, created_at)`. Mirrors `voice_runtime_versions` (migration 151) shape/conventions. No new permission migration (reuse `voice::admin::*`).
- **ITEM-2**: `VoiceModelRow` + `VoiceModelRepository` (CRUD: list, get_by_id, get_by_filename, insert, delete) in `voice/repository.rs` + a `voice/models.rs` DTO (`VoiceModel`, `VoiceModelSource`). Mirror `runtime_version` repository idioms.
- **ITEM-3**: Curated model **catalog** constant in `voice/model.rs` (`CATALOG: &[CatalogModel]` with name, filename, size, pinned sha256, `quantization`, `multilingual`/`english_only`) covering tiny/base/small/medium/large-v3/large-v3-turbo + `.en` + `q5_1`/`q8_0`. Keep `KNOWN_MODEL_SHA256` fail-closed for catalog entries; expose `catalog()` + `catalog_lookup(name)`.

### Backend — download (async SSE, mirror runtime_version)
- **ITEM-4**: `voice/model_download_task.rs` — mirror of `runtime_version/download_task.rs`:
  `static MODEL_DOWNLOAD_TASKS: Lazy<DashMap<String, Arc<ModelDownloadTask>>>` keyed by target filename; per-task `broadcast::Sender<SSEModelDownloadEvent>` (`sse_event_enum!` → `Connected/Progress/Complete/Failed`, progress = `bytes_received/total_bytes/percent`); `start_or_join`, `spawn_runner`, `SHUTDOWN` Notify + `shutdown_all()` (wired into main shutdown next to voice runtime-version `shutdown_all`).
- **ITEM-5**: Download runner supports BOTH sources via a unified single-file streaming path in `model.rs`: catalog → pinned URL + verify-against-pin (fail-closed); arbitrary → user URL/HF-repo+filename, **SSRF-validated** (`OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS`, debug `WHISPER_MODEL_MIRROR`/`DEV_LOCAL` seam retained), sha256 **computed** (not required), `verified=false`. Magic-byte validation (whisper ggml magic `0x67676d6c` / GGUF `0x46554747`) before commit. On success: register a `voice_models` row + `sync_publish(VoiceModel, Create)`.
- **ITEM-6**: Download REST (`instance_handlers.rs`/new `model_handlers.rs`, `voice::admin::manage` for start, `admin::read` for read):
  `POST /voice/models/download` (body: `{name, source: 'catalog'|'url', repository?, filename?, url?}` → `DownloadVersionStartedResponse`-shaped `{task_id,key,events_url}`),
  `GET /voice/models/downloads` (active snapshots, reload-repaint),
  `GET /voice/models/downloads/{key}/events` (SSE — subscribe-before-snapshot, replay, live).

### Backend — library ops + upload
- **ITEM-7**: Installed-model REST (`voice::admin::read`/`manage`):
  `GET /voice/models` (list installed rows), `DELETE /voice/models/{id}` (delete row + on-disk file; refuse/za­guard deleting the active model unless it's the last and acked), `POST /voice/models/{id}/activate` (set `settings.model` → the row's name; triggers existing drain+respawn via `auto_start::live_handle_if_current`). Each mutation `sync_publish(VoiceModel|VoiceSettings)`.
- **ITEM-8**: Upload REST `POST /voice/models/upload` (`voice::admin::manage`, multipart, per-route `DefaultBodyLimit` raised): fields `file` + `name`; magic-byte validate (ITEM-5 helper), size cap (`VOICE_MODEL_MAX_UPLOAD_BYTES`), stream to temp then atomic rename into `voice-models/`, compute sha256 (`verified=false`, `source='upload'`), insert row + `sync_publish(VoiceModel, Create)`. Mirror `file/handlers/upload.rs` + `llm_model/handlers/uploads.rs` (buffer-to-temp, validate, commit).
- **ITEM-9**: Relax `validate_settings_patch` (`handlers.rs`) so `settings.model` accepts **any installed model name** (catalog names ∪ `voice_models.name`), not just the 4-const; update `model::resolve`/`ensure_model`/`auto_start` to resolve the active model from the installed set (catalog auto-download still allowed as a fallback for a catalog name). Keep `GET /voice/model/status` (used by the not-ready banner). Remove the old synchronous `POST /voice/model/download` (superseded) and migrate its callers/tests to the async path.
- **ITEM-10**: `SyncEntity::VoiceModel` added to `sync/event.rs` (next to `VoiceRuntimeVersion`); owner-less admin audience `Audience::perm::<VoiceAdminRead>()` (mirror the runtime-version emit at `download_task.rs:339`).

### Frontend — stores (mirror voice engine-version + llm upload)
- **ITEM-11**: `VoiceModelDownloadProgress.store.ts` — mirror `VoiceDownloadProgress.store.ts` (SSE `activeByKey` map keyed by filename, `startDownload`, `subscribeToKey` with `claimSubscription` dedupe, `loadActive()` reload-reattach, self-gated on `VoiceAdminRead`).
- **ITEM-12**: Extend `VoiceModel.store.ts` — add `listInstalled()`, `deleteModel(id)`, `activateModel(id)`, catalog fetch; subscribe `on('sync:voice_model', reload)` + keep `sync:voice_settings`.
- **ITEM-13**: `VoiceModelUpload.store.ts` — mirror `LlmModelUpload.store.ts` (XHR `FormData`, per-file + overall progress, `cancelUpload`), calling `ApiClient.Voice.uploadModel` (new).

### Frontend — surfaces (replace single ModelCard)
- **ITEM-14**: `AvailableModelsCard.tsx` — mirror `AvailableVersionsCard.tsx`: catalog rows (name, size, quantization/lang tags, `installed`/`latest` tags), per-row Install button + inline `DownloadProgressLine` (reuse the byte/percent progress + `formatBytes` caption), gated `VoiceAdminManage`. Plus an **"Add from URL / HF repo"** affordance (small form/drawer) for the arbitrary-source download, with an explicit "unverified" note.
- **ITEM-15**: `InstalledModelsCard.tsx` — mirror `InstalledVersionsCard.tsx`: installed rows with source + verified/unverified tags, **Set active** (Star, mirror set-default) + **Delete** (`Confirm`, mirror), gated `VoiceAdminManage`; active row badge.
- **ITEM-16**: `UploadModelDrawer.tsx` — mirror `AddLocalLlmModelUploadDrawer.tsx`: kit `<Upload accept=".bin,.gguf">` + name field + per-file/overall `<Progress>` + Cancel, driven by `VoiceModelUpload.store` + a `UploadModelDrawer` open-store.
- **ITEM-17**: Wire into `VoiceSettingsPage.tsx`: replace `<ModelCard/>` with `<AvailableModelsCard/>` + `<InstalledModelsCard/>` + mount `<UploadModelDrawer/>` (upload trigger in the Available card `extra`, mirroring the LLM add-model dropdown). Update the not-ready banner logic to use installed-set presence.
- **ITEM-18**: Pagination — both catalog and installed lists use the shared numbered `ListPagination` (pageSize 10) per the settings-list idiom, with the "N of M" summary. (Client-side over the bounded catalog constant + installed list; both are low-cardinality but the gate/idiom require bounded render + N-of-M.)
- **ITEM-19**: `api-client` regen (`just openapi-regen` → BOTH `ui/` + `desktop/ui/`), new `Voice.*` methods (uploadModel, listModels, deleteModel, activateModel, downloadModel(plural), listModelDownloads, subscribeModelDownloadEvents).

### Cross-cutting
- **ITEM-20**: `VOICE_MODEL_MAX_UPLOAD_BYTES` limit (see DECISIONS: fixed constant w/ rationale — whisper models are upstream-bounded ~3 GB) as a named const in `voice/model.rs`, structured for later promotion to a settings column.
- **ITEM-21**: Responsive: all new cards/rows use the voice house style (`Flex … wrap` + `flex-1 min-w-48`), no breakpoints; verify at 390 px. Gallery: follow the existing voice **DRIFT-1 pending** convention in `dev/gallery/coverage.ts` (voice surfaces are e2e-covered, not gallery cells) — add `coverage.ts` entries for the new cards/drawer marked pending→e2e, and (if a gallery overlay is warranted for the upload drawer) an `overlays.tsx` entry + fixture mirroring `overlay-add-local-llm-model-upload-drawer`.

## Files to touch

### Backend (`src-app/server/`)
- `migrations/00000000000155_create_voice_models.sql` (new)
- `src/modules/voice/model.rs` (catalog, unified streaming download, magic validate, SSRF, limit const)
- `src/modules/voice/model_download_task.rs` (new — async SSE task registry)
- `src/modules/voice/models.rs` (DTOs: `VoiceModel`, `VoiceModelSource`, requests/responses)
- `src/modules/voice/repository.rs` (`VoiceModelRepository`)
- `src/modules/voice/model_handlers.rs` (new) or extend `instance_handlers.rs` (model REST)
- `src/modules/voice/routes.rs` (wire the new `/voice/models/*` sub-router)
- `src/modules/voice/handlers.rs` (relax `validate_settings_patch`)
- `src/modules/voice/auto_start.rs` + `deployment/local.rs` (resolve active model from installed set)
- `src/modules/voice/mod.rs` (register repo/router; wire `model_download_task::shutdown_all`)
- `src/modules/sync/event.rs` (`VoiceModel` entity)
- `src/main.rs` (shutdown hook, if not centralized in voice `mod.rs`)
- `openapi/openapi.json` + `src/api-client/types.ts` (regen — generated, excluded from coverage law)

### Frontend (`src-app/ui/`)
- `src/modules/voice/components/AvailableModelsCard.tsx` (new)
- `src/modules/voice/components/InstalledModelsCard.tsx` (new)
- `src/modules/voice/components/UploadModelDrawer.tsx` (new)
- `src/modules/voice/components/VoiceSettingsPage.tsx` (rewire)
- `src/modules/voice/components/ModelCard.tsx` (removed/absorbed)
- `src/modules/voice/stores/VoiceModelDownloadProgress.store.ts` (new)
- `src/modules/voice/stores/VoiceModelUpload.store.ts` (new)
- `src/modules/voice/stores/VoiceModel.store.ts` (extend)
- `src/modules/voice/stores/index.ts`, `src/modules/voice/module.tsx` (register)
- `src/modules/voice/types.ts` (declmerge new types)
- `src/api-client/types.ts` (regen)
- `src/dev/gallery/coverage.ts` (+ maybe `overlays.tsx` + `fixtures/voice.ts`)
- Desktop mirror: `src-app/desktop/ui/src/api-client/types.ts` (regen); diff any hand-written voice overrides (R2-3).

### Tests
- Unit: `voice/model.rs`, `voice/model_download_task.rs`, `voice/repository.rs`, `voice/handlers.rs` `#[cfg(test)]`; `stores/downloadProgress.helpers.test.ts`-style + store reducers.
- Integration: `src-app/server/tests/voice/` (new `model_management_test.rs`).
- E2e: `src-app/ui/tests/e2e/14-voice/` (extend `voice-settings-admin.spec.ts`; new `voice-model-mgmt.spec.ts`); `voice-helpers.ts` mock routes.

## Patterns to follow (closest existing module per area)
- Async SSE download (task registry, broadcast, `/events`, start_or_join, shutdown) → **`voice/runtime_version/download_task.rs`** + its handlers (`runtime_version/handlers.rs`).
- Installed-DB-row + list/set-default/delete → **`voice_runtime_versions`** table + `runtime_version/{repository,handlers}.rs` + `InstalledVersionsCard.tsx`.
- Available list + install + inline progress → **`AvailableVersionsCard.tsx`** + `VoiceDownloadProgress.store.ts` + `downloadProgress.helpers.ts`.
- Multipart upload (validate → temp → commit) → **`file/handlers/upload.rs`** + **`llm_model/handlers/uploads.rs`**.
- Upload UI (kit `<Upload>` + XHR per-file progress + drawer) → **`AddLocalLlmModelUploadDrawer.tsx`** + `LlmModelUpload.store.ts` + `api-client/core.ts` XHR path.
- SSRF for arbitrary URL → **`utils/url_validator.rs` `PUBLIC_HTTP_OR_HTTPS`** (as web_search/lit_search page-fetch use it).
- Sync entity emit (admin audience) → **`runtime_version/download_task.rs:339`** (`VoiceRuntimeVersion` Create).
- Numbered pagination → **`common/ListPagination.tsx`** as used in `LlmRepositorySettings.tsx`.
- Permission gating (reuse, no new perm) → route `VoiceAdminRead` + `<Can VoiceAdminManage>` as in `ModelCard.tsx`/`AvailableVersionsCard.tsx`.

## UI-surface checklist (per new surface)

### AvailableModelsCard (twin of AvailableVersionsCard)
- **Precedent**: `AvailableVersionsCard.tsx` — same Card+extra+rows+inline `DownloadProgressLine` structure/tokens; add lang/quantization tags. Divergence from it is a bug.
- **Scale/cardinality**: catalog is a fixed constant (~24 entries with quantized). Bounded. Render via `ListPagination` (pageSize 10) + "N of M". Never render-all beyond a page.
- **Device**: `Flex … wrap` house style; verify 390 px (tags/buttons wrap, no h-scroll), mirrors the sibling. Narrow state exercised in e2e (voice is gallery-DRIFT-1).
- **Progress**: per-row byte/percent progress bar via the SSE snapshot (not an indeterminate spinner) — the whole point of the async upgrade.

### InstalledModelsCard (twin of InstalledVersionsCard)
- **Precedent**: `InstalledVersionsCard.tsx` — Descriptions row + Set-active(Star)/Delete(Confirm), source+verified tags. Mirror exactly.
- **Scale**: installed set is low-cardinality (a handful, realistically <30). `ListPagination` (pageSize 10) + "N of M"; server returns the bounded list.
- **Device**: `flex items-center gap-2 flex-wrap` + `flex-1 min-w-48` like the sibling; 390 px verified.
- **Progress**: N/A (terminal rows); Delete shows a Confirm; activate is instantaneous + sync-refetched.

### UploadModelDrawer (twin of AddLocalLlmModelUploadDrawer)
- **Precedent**: `AddLocalLlmModelUploadDrawer.tsx` — kit `<Upload>` + Selected-files card + per-file/overall `<Progress>` + Cancel. Mirror structure/tokens.
- **Scale**: single-file upload (one whisper .bin); trivially bounded.
- **Device**: Drawer is full-height responsive by construction; content uses `Flex … wrap`; 390 px verified.
- **Progress**: real XHR byte progress (per-file + overall) + itemized error tone — not a boolean spinner.
