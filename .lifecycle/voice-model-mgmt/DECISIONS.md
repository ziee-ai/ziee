# DECISIONS — voice-model-mgmt

All product/human inputs resolved before implementation. No unresolved markers remain.

### DEC-1: Where can users download whisper models from?
**Resolution:** Both a curated **sha256-pinned catalog** AND **arbitrary HF-repo/URL**. Catalog entries stay fail-closed (must match their pin). Arbitrary (non-catalog) downloads are SSRF-validated, sha256 is computed (not required), and the model is stored `verified=false`.
**Basis:** user — plan-time AskUserQuestion ("Catalog + arbitrary HF repo/URL").

### DEC-2: Multi-model library or single configured model?
**Resolution:** Multi-model **library** — install/upload many; an Installed list with Set-active + Delete; the active one drives `voice_runtime_settings.model`.
**Basis:** user — plan-time AskUserQuestion ("Library + pick active").

### DEC-3: How broad is the downloadable catalog?
**Resolution:** standard + turbo + `.en` + **quantized** (`q5_1`/`q8_0`): tiny, tiny.en, base, base.en, small, small.en, medium, medium.en, large-v3, large-v3-turbo, plus q5_1/q8_0 variants where whisper.cpp publishes them.
**Basis:** user — plan-time AskUserQuestion ("Also quantized variants").

### DEC-4: New `voice::models::*` permission, or reuse `voice::admin::{read,manage}`?
**Resolution:** **Reuse** `voice::admin::{read,manage}` — no new permission constant, no new grant migration. Model reads gate on `admin::read`, mutations on `admin::manage`.
**Basis:** convention — the voice module deliberately uses a `transcribe` + `admin::{read,manage}` split rather than per-resource perms (`voice/permissions.rs:4-6` docstring); `VoiceAdminManage`'s doc already says "download models". Matches [[feedback_match_existing_patterns]]. (Consequence: no A10 new-permission obligation; TEST-24 still adds a restricted-user check defensively.)

### DEC-5: Track installed models in a DB table, or derive from disk?
**Resolution:** A `voice_models` **DB table** (migration 155). Download-complete and upload insert a row; delete removes it; the list endpoint reads it.
**Basis:** convention — everything installable in this domain gets a DB row (`voice_runtime_versions` for engine binaries, `llm_models` for LLM weights). A table gives stable identity for async tasks, provenance (`source`), `verified`, and sha256 that disk-scraping can't. Mirrors `voice_runtime_versions` (migration 151).

### DEC-6 (configurable-settings, MANDATORY): Is the upload size cap a fixed constant or an admin setting?
**Resolution:** **Fixed named constant** `VOICE_MODEL_MAX_UPLOAD_BYTES` in `voice/model.rs` (proposed 5 GiB), structured as a const (not an inline magic number) so it can be promoted to a `voice_runtime_settings` column later without a rewrite.
**Basis:** convention + explicit rationale — whisper model files are **upstream-bounded** (largest, large-v3, is ~3.1 GB; quantized are smaller), unlike open-ended user documents. This is a safety bound, not a per-deployment operational tunable an admin needs to tune; `llm_model` likewise uses a fixed `MAX_MODEL_UPLOAD_BYTES` const. The existing `voice_runtime_settings.max_upload_bytes` governs *transcription clips*, a different concern — not reused here.

### DEC-7 (configurable-settings): Is the catalog membership a settings row or code?
**Resolution:** **Code constant** (`CATALOG` in `voice/model.rs`). Adding/removing a catalog model = a code change that also pins its sha256.
**Basis:** convention — mirrors the existing `SUPPORTED_MODELS`/`KNOWN_MODEL_SHA256` constants and the engine-version upstream catalog. Pins are a security artifact that belongs in code review, not an admin-editable row.

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

### DEC-12: Handling of unverified (arbitrary/upload) models?
**Resolution:** Store `verified=false` + `source ∈ {url,upload}`; surface an "unverified" tag in InstalledModelsCard and a note in the add-from-URL / upload UI. Catalog models are `verified=true`. Activation/serving is allowed regardless (admin's choice).
**Basis:** user (DEC-1 approved the security-posture change) — fail-closed retained for known models, opt-in unverified for custom, clearly labeled.

### DEC-13: Active-model deletion guard?
**Resolution:** `DELETE /voice/models/{id}` refuses when the row is the active model unless an explicit ack flag is passed (mirrors InstalledVersionsCard's default-ack Confirm); the UI steers the user to activate another model first.
**Basis:** codebase — the running whisper-server is pointed at the active model file; deleting it out from under a live instance would break transcription. Mirrors the runtime-version delete-default guard.

### DEC-14: Model name/filename constraints?
**Resolution:** Custom model `name` ≤ 50 chars (fits `voice_runtime_settings.model VARCHAR(50)`), sanitized to a safe slug; stored filename is `ggml-<name>.bin` (or the uploaded/`.gguf` filename) unique in `voice_models.filename`. Path-traversal rejected (mirror the runtime-version download validation).
**Basis:** codebase — `settings.model` column width + the existing `model_filename()` convention.

### DEC-15: Gallery coverage convention for the new voice surfaces?
**Resolution:** Follow the existing voice **DRIFT-1 pending** convention in `dev/gallery/coverage.ts` (voice surfaces are e2e-covered, not gallery cells). Add pending coverage entries; add an `overlays.tsx` cell + fixture for the upload drawer ONLY if `check:gallery-coverage`/`check:state-matrix` require it.
**Basis:** codebase — every current voice surface is `kind:'pending'` deferred to the `14-voice` e2e specs (`coverage.ts:514-519`). Introducing bespoke gallery cells would diverge from the module's convention (B3 — don't fight shared harness).
