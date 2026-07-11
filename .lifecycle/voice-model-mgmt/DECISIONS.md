# DECISIONS — voice-model-mgmt

All product/human inputs resolved before implementation. No unresolved markers remain.

### DEC-1: Where can users download whisper models from?
**Resolution:** A **runtime-fetched HF catalog** (the `ggerganov/whisper.cpp` model list, fetched live) AND **arbitrary HF-repo/URL**. Catalog/HF downloads are verified against HF's advertised git-LFS `oid` (= sha256) → `verified=true` on match. Truly-arbitrary non-HF URLs are SSRF-validated, sha256 computed (not checkable against a source of truth), stored `verified=false`.
**Basis:** user — plan-time AskUserQuestion #1 ("Catalog + arbitrary HF repo/URL") + follow-up #2 ("Fetch catalog list from HF at runtime") + concern "what if they change / new versions" → dynamic list so new upstream models appear automatically. [[feedback_match_existing_patterns]] — mirrors the engine-version upstream-fetch sibling.

### DEC-2: Multi-model library or single configured model?
**Resolution:** Multi-model **library** — install/upload many; an Installed list with Set-active + Delete; the active one drives `voice_runtime_settings.model`.
**Basis:** user — plan-time AskUserQuestion ("Library + pick active").

### DEC-3: How broad is the downloadable catalog?
**Resolution:** **Whatever `ggerganov/whisper.cpp` currently publishes** — the runtime fetch lists all `ggml-*.bin` model files (multilingual, `.en`, AND quantized `q5_*`/`q8_*`), so the standard+turbo+.en+quantized set is covered AND any newly-published variant appears automatically. A light filter excludes obvious non-model files; no fixed enumeration to maintain.
**Basis:** user — "Also quantized variants" + "what if they change / new versions" (dynamic list). Supersedes the earlier fixed-enumeration reading now that DEC-7 is runtime-fetch.

### DEC-4: New `voice::models::*` permission, or reuse `voice::admin::{read,manage}`?
**Resolution:** **Reuse** `voice::admin::{read,manage}` — no new permission constant, no new grant migration. Model reads gate on `admin::read`, mutations on `admin::manage`.
**Basis:** convention — the voice module deliberately uses a `transcribe` + `admin::{read,manage}` split rather than per-resource perms (`voice/permissions.rs:4-6` docstring); `VoiceAdminManage`'s doc already says "download models". Matches [[feedback_match_existing_patterns]]. (Consequence: no A10 new-permission obligation; TEST-24 still adds a restricted-user check defensively.)

### DEC-5: Track installed models in a DB table, or derive from disk?
**Resolution:** A `voice_models` **DB table** (migration 155). Download-complete and upload insert a row; delete removes it; the list endpoint reads it.
**Basis:** convention — everything installable in this domain gets a DB row (`voice_runtime_versions` for engine binaries, `llm_models` for LLM weights). A table gives stable identity for async tasks, provenance (`source`), `verified`, and sha256 that disk-scraping can't. Mirrors `voice_runtime_versions` (migration 151).

### DEC-6 (configurable-settings, MANDATORY): Is the upload size cap a fixed constant or an admin setting?
**Resolution:** **Fixed named constant** `VOICE_MODEL_MAX_UPLOAD_BYTES` in `voice/model.rs` (proposed 5 GiB), structured as a const (not an inline magic number) so it can be promoted to a `voice_runtime_settings` column later without a rewrite.
**Basis:** convention + explicit rationale — whisper model files are **upstream-bounded** (largest, large-v3, is ~3.1 GB; quantized are smaller), unlike open-ended user documents. This is a safety bound, not a per-deployment operational tunable an admin needs to tune; `llm_model` likewise uses a fixed `MAX_MODEL_UPLOAD_BYTES` const. The existing `voice_runtime_settings.max_upload_bytes` governs *transcription clips*, a different concern — not reused here.

### DEC-7 (configurable-settings): Is the catalog membership a settings row, code, or runtime fetch?
**Resolution:** **Runtime fetch from HuggingFace** — `voice/model_catalog.rs` calls the HF tree/model API for `ggerganov/whisper.cpp` (fixed, trusted host), filters `ggml-*.bin`, and reads each file's LFS `oid` (sha256) + size. The list is cached with a short TTL and refreshed via a "Check for updates" affordance (mirrors `VoiceUpdate.checkForUpdates()`). NOT a code constant and NOT an admin-editable table. The HF repo id is a fixed constant (overridable via a debug-only test seam), so it is not an admin-tunable attack surface.
**Basis:** user (DEC-1 follow-up) + convention — this is exactly how the engine-version "available" list works (upstream GitHub API at runtime; installed = DB rows). Verification uses the HF-advertised LFS oid, so integrity is checked against the source of truth even without a maintained in-code pin. Connected-only (air-gapped boxes get no catalog list but upload + installed-model use still work). Supersedes the earlier hardcoded-constant plan.

### DEC-8 (configurable-settings): Is there a download-concurrency cap?
**Resolution:** No explicit cap. Per-model dedupe via `start_or_join` (one runner per target filename); distinct models may download concurrently.
**Basis:** convention — identical to the engine-version download task (`runtime_version/download_task.rs`), which caps nothing and dedupes per key.

### DEC-9: SSRF policy for arbitrary-URL downloads?
**Resolution:** `OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS` (blocks loopback/RFC1918/IMDS, re-validates redirects). The debug-only `WHISPER_MODEL_MIRROR`/`DEV_LOCAL` seam (compiled out of release) is the loopback test path.
**Basis:** convention — the same policy web_search/lit_search apply to user/model-supplied fetch URLs (`utils/url_validator.rs`). Admin-only gating narrows but does not remove the SSRF requirement.

### DEC-10: Arbitrary-download input shape — full git-clone (like LLM provider) or single-file?
**Resolution:** **Single-file streaming.** The request accepts either an HF `repository` + `filename` (constructed to a `.../resolve/main/<file>` URL) or a raw `url`. No git-clone/LFS.
**Basis:** codebase — whisper models are single `ggml-*.bin`/`*.gguf` files; the existing `model.rs` already streams single files. The LLM-provider git-clone+LFS path exists for multi-file safetensors repos, which does not apply here.

### DEC-11: Keep or remove the synchronous `POST /voice/model/download`?
**Resolution:** **Remove** it; the async `POST /voice/models/download` + SSE supersedes it. Keep `GET /voice/model/status` (not-ready banner). Migrate the frontend store + the `voice-settings-admin` e2e assertion to the async flow (TEST-21).
**Basis:** codebase — a synchronous multi-GB download that blocks the request is the exact wart this feature fixes; the endpoint has one caller, migrated in-change. A5 shrink-guard honored by re-pointing the old test, not deleting it.

### DEC-12: Handling of verified vs unverified models?
**Resolution:** A model is `verified=true` when its downloaded bytes match HF's advertised git-LFS `oid` (sha256) — i.e. catalog picks and HF-repo downloads. Truly-arbitrary non-HF URL downloads and uploads have no source-of-truth digest → `verified=false`, with an "unverified" tag in InstalledModelsCard + a note in the add-from-URL / upload UI. Activation/serving is allowed regardless (admin's choice). Recorded `sha256` is stored for every model (for update-detection, DEC-16).
**Basis:** user (DEC-1) — integrity checked against HF's oid where a source of truth exists; opt-in unverified for custom, clearly labeled.

### DEC-13: Active-model deletion guard?
**Resolution:** `DELETE /voice/models/{id}` refuses when the row is the active model unless an explicit ack flag is passed (mirrors InstalledVersionsCard's default-ack Confirm); the UI steers the user to activate another model first.
**Basis:** codebase — the running whisper-server is pointed at the active model file; deleting it out from under a live instance would break transcription. Mirrors the runtime-version delete-default guard.

### DEC-14: Model name/filename constraints?
**Resolution:** Custom model `name` ≤ 50 chars (fits `voice_runtime_settings.model VARCHAR(50)`), sanitized to a safe slug; stored filename is `ggml-<name>.bin` (or the uploaded/`.gguf` filename) unique in `voice_models.filename`. Path-traversal rejected (mirror the runtime-version download validation).
**Basis:** codebase — `settings.model` column width + the existing `model_filename()` convention.

### DEC-15: Gallery coverage convention for the new voice surfaces?
**Resolution:** Follow the existing voice **DRIFT-1 pending** convention in `dev/gallery/coverage.ts` (voice surfaces are e2e-covered, not gallery cells). Add pending coverage entries; add an `overlays.tsx` cell + fixture for the upload drawer ONLY if `check:gallery-coverage`/`check:state-matrix` require it.
**Basis:** codebase — every current voice surface is `kind:'pending'` deferred to the `14-voice` e2e specs (`coverage.ts:514-519`). Introducing bespoke gallery cells would diverge from the module's convention (B3 — don't fight shared harness).

### DEC-16: What happens when upstream models change or new versions are published?
**Resolution:** Because the Available list is fetched live (DEC-7), **new upstream models appear automatically** on the next fetch/"Check for updates". For an already-**installed** model, its recorded `sha256` is compared against the current upstream HF `oid`; a mismatch surfaces an **"update available"** tag on the Installed row, and re-downloading replaces the file + updates the row (mirrors the engine-version `check-updates` → update-available flow). No auto-update — the admin chooses to re-download.
**Basis:** user — "what if they change and have new versions?" + convention (`VoiceUpdate.checkForUpdates()` + `check_updates` endpoint already implement this shape for engine binaries).

### DEC-17 (configurable-settings, MANDATORY): Is the model source repo fixed or admin-configurable?
**Resolution:** **Admin-configurable** — `voice_runtime_settings.model_source_repo` (default `ggerganov/whisper.cpp`), surfaced through the EXISTING `GET/PUT /voice/settings` + `VoiceSettings` sync + `validate_settings_patch` (no new endpoint/table). If upstream renames/moves the repo, or an operator wants an internal HF mirror (air-gapped), they repoint the field — no code release. Fetch failures **graceful-degrade** (empty Available list + "source unreachable" state; upload + arbitrary-URL + installed models unaffected). The admin-configured source is treated as **trusted** for the catalog list/download (may be internal/loopback — a distinct trust boundary from the user-supplied arbitrary URL, which stays `PUBLIC_HTTP_OR_HTTPS`).
**Basis:** user — plan-time AskUserQuestion #3 ("Admin-configurable, default upstream"). This is a genuine operational tunable (unlike the sha256 digests, which are security artifacts), so the configurable-settings rule's default (admin-configurable) applies. Two-trust-boundary split mirrors `web_search` (trusted SearXNG base vs strict page-fetch).
