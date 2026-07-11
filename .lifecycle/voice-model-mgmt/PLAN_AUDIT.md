# PLAN_AUDIT — voice-model-mgmt

Audit of PLAN.md against the current codebase (surveyed), before writing code.

## Breakage risk
- **Removing synchronous `POST /voice/model/download`** (ITEM-9): current callers are the
  frontend `VoiceModel.store.downloadModel()` and integration/e2e (`voice-settings-admin.spec`
  TEST-29 clicks `voice-model-download-btn`). Removing it breaks those unless migrated in the
  SAME change. Mitigation: ITEM-11..17 migrate the UI to the async flow; the e2e/integration
  for the old endpoint are rewritten (A5: TEST-IDs must not vanish → the old download TEST is
  *re-pointed* to the async flow, not deleted). Keep `GET /voice/model/status` (used by the
  not-ready banner) untouched.
- **Relaxing `validate_settings_patch`** (ITEM-9): today it rejects any model not in the
  4-const allow-list (`handlers.rs:295-301` tests). Widening to "any installed model" must not
  regress the "unsupported model rejected" behavior — the new rule is "installed OR a known
  catalog name", and a genuinely unknown string is still 400. The existing test at
  `handlers.rs:301` must be updated to assert an unknown-and-not-installed model is rejected.
- **`settings.model` column is `VARCHAR(50)`** (migration 151:64). Uploaded/custom model
  *names* must fit 50 chars — validate name length on upload/activate. Filenames
  (`ggml-<name>.bin`) live in `voice_models.filename`; the active pointer stays the short name.
- **Active-model deletion**: deleting the currently-active model would leave the engine pointing
  at a missing file. Guard: `DELETE /voice/models/{id}` refuses when the row is active unless the
  client acks, and activation of another model must precede (mirrors InstalledVersionsCard's
  default-ack Confirm).

## Pattern conformance
- Async SSE download (ITEM-4/5/6) mirrors `runtime_version/download_task.rs` 1:1 (DashMap +
  `broadcast` + `sse_event_enum!` + `start_or_join` + `SHUTDOWN`/`shutdown_all` + subscribe-
  before-snapshot handler). PASS — this is a proven second-instantiation (voice already copied
  it once from llm_local_runtime).
- Upload (ITEM-8) mirrors `file/handlers/upload.rs` (validate → temp → commit → magic sniff) and
  `llm_model/handlers/uploads.rs` (multipart loop, per-route `DefaultBodyLimit`). PASS.
- Sync entity (ITEM-10) mirrors the `VoiceRuntimeVersion` variant + admin-audience emit. PASS.
- Stores (ITEM-11/13) mirror `VoiceDownloadProgress.store.ts` / `LlmModelUpload.store.ts`
  verbatim in shape (SSE map / XHR progress). PASS.
- Cards (ITEM-14/15/16) mirror `AvailableVersionsCard`/`InstalledVersionsCard`/
  `AddLocalLlmModelUploadDrawer`. PASS.
- **Pagination divergence (CONCERN, resolved)**: the direct sibling `AvailableVersionsCard`
  uses `slice(0,10)` + "+N hidden", NOT `ListPagination`. PLAN chooses `ListPagination` (the
  settings-list idiom the Phase-1 checklist prescribes). This is a *deliberate* divergence from
  the immediate sibling toward the house settings-list idiom — recorded so the precedent-
  fidelity audit angle doesn't flag it as accidental. Acceptable; both are "bounded render".
- **Permissions**: reuse `voice::admin::{read,manage}` (no new permission) — conforms to the
  voice module's deliberate `admin::{read,manage}` (not per-resource) convention
  (`permissions.rs:4-6` docstring). No A10 new-permission obligation is triggered.

## Migration collisions
- New migration `155` — `ls migrations/ | tail -1` = `154`. No collision. build.rs will pick it
  up (run `cargo clean -p ziee` if "relation does not exist" appears after adding it).
- `voice_models` table name — grep confirms no existing table by that name. No clash with the
  unused `llm_model_files`/`llm_download_instances` (different module).

## OpenAPI regen
- **Required in BOTH workspaces** (new `Voice.*` endpoints + types): `just openapi-regen`.
  The golden parity test (`openapi::emit_ts::tests::types_ts_parity`) enforces it. Desktop
  api-client regenerates identically. Generated files excluded from coverage law + frontend
  gates.

## SSRF / security (arbitrary-URL download)
- Arbitrary-URL/HF download (ITEM-5) is the one genuinely new outbound-fetch-from-user-input
  surface. It MUST route through `OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS` (blocks
  loopback/RFC1918/IMDS, re-validates redirects) exactly as web_search/lit_search page-fetch do.
  Admin-only (`voice::admin::manage`) narrows exposure but does not remove the SSRF need.
  Catalog downloads use the fixed HF base URL (fail-closed pin) and are not user-controlled.
  The debug `WHISPER_MODEL_MIRROR`/`DEV_LOCAL` seam (compiled out of release) provides the
  loopback test path — same pattern as `WEB_SEARCH_BRAVE_ENDPOINT`.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — new table mirrors `voice_runtime_versions` (mig 151); no collision.
- **ITEM-2** — verdict: PASS — repository/DTO mirror `runtime_version` idioms.
- **ITEM-3** — verdict: PASS — runtime HF catalog fetch mirrors the engine-version upstream-fetch sibling; reads the admin-configured source (DEC-7/17); graceful-degrades on failure; oid = sha256 for verification.
- **ITEM-4** — verdict: PASS — direct mirror of `runtime_version/download_task.rs`.
- **ITEM-5** — verdict: CONCERN — two-trust-boundary fetch is the security-critical part: admin-configured source = trusted (internal mirror allowed), user arbitrary URL = `PUBLIC_HTTP_OR_HTTPS`; verify against HF oid where available, magic-byte validate always. Resolved by the web_search-style split above; enumerated as its own tests (TEST-3/8/9).
- **ITEM-6** — verdict: PASS — endpoints mirror `/voice/versions/*` download trio.
- **ITEM-7** — verdict: CONCERN — active-model delete/activate guard (breakage risk above); resolved with the ack guard + activate-before-delete rule.
- **ITEM-8** — verdict: PASS — mirrors file/llm_model multipart upload; needs per-route body limit + name-length validation.
- **ITEM-9** — verdict: CONCERN — removes an endpoint + relaxes settings validation (breakage risk above); resolved by migrating callers/tests in-change and keeping the unknown-model 400.
- **ITEM-10** — verdict: PASS — additive sync entity + admin-audience emit.
- **ITEM-11** — verdict: PASS — mirrors `VoiceDownloadProgress.store.ts`.
- **ITEM-12** — verdict: PASS — additive store actions + sync subscription.
- **ITEM-13** — verdict: PASS — mirrors `LlmModelUpload.store.ts` (XHR progress path exists in `api-client/core.ts`).
- **ITEM-14** — verdict: PASS — mirrors `AvailableVersionsCard`; arbitrary-source affordance is additive.
- **ITEM-15** — verdict: PASS — mirrors `InstalledVersionsCard`.
- **ITEM-16** — verdict: PASS — mirrors `AddLocalLlmModelUploadDrawer`.
- **ITEM-17** — verdict: CONCERN — replacing `ModelCard` changes the not-ready banner input (was single-model status; now installed-set). Resolved: banner reads "active model present in installed set".
- **ITEM-18** — verdict: PASS — `ListPagination` reuse (deliberate idiom divergence noted above).
- **ITEM-19** — verdict: PASS — `just openapi-regen` both workspaces; golden parity test guards it.
- **ITEM-20** — verdict: PASS — fixed limit const w/ rationale (see DECISIONS); structured for later promotion.
- **ITEM-21** — verdict: CONCERN — voice is gallery-DRIFT-1 (e2e-covered, no cells). Must follow that convention (coverage.ts pending entries + rely on e2e) rather than force gallery cells; Phase-8 `gate:ui`/state-matrix must still pass — resolved by matching the existing voice `coverage.ts` pending pattern and adding an overlay+fixture only for the upload drawer if `check:gallery-coverage` demands it.

- **ITEM-22** — verdict: PASS — `model_source_repo` rides the EXISTING `/voice/settings` GET/PUT + `VoiceSettings` sync + `validate_settings_patch`; only an additive column (mig 155 ALTER) + a validation clause + one settings field. No new endpoint.
- **ITEM-23** — verdict: PASS — `VoiceModelUpdate.store` mirrors `VoiceUpdate.store`; update-detection compares recorded sha256 vs upstream oid (additive); graceful-degrade is an empty/error render state, not new control flow.

## Migration collisions (revised)
- Migration `155` now does three things: create `voice_models`, `ALTER voice_runtime_settings ADD
  model_source_repo`, `ALTER voice_runtime_instance ADD CONSTRAINT CHECK (state IN (...))`. Still a
  single new file at the next free number (154→155). The CHECK ALTER is safe: the singleton row's
  existing `state` is always one of the seven valid names (written only by the state machine), so
  no pre-existing row violates it. Additive column has a DEFAULT (no backfill). `cargo clean -p ziee`
  after adding it.

## Runtime parity items (FB-2 — ITEM-24..33)
- **ITEM-24** — verdict: CONCERN — touches the live crash-recovery path; must ADD hot-path `Crashed`
  + backoff WITHOUT regressing the existing pre-healthy-crash protection or the `is_failed()` gate.
  Resolved: mirror `probe_liveness` exactly + a supervision integration test (TEST-28).
- **ITEM-25** — verdict: PASS — the async task already mirrors the SHUTDOWN-race sibling; adds a
  cancel token + moves `.tmp` cleanup off the Err-only branch.
- **ITEM-26** — verdict: CONCERN — drain-before-respawn on the model-switch path is a behavior
  change to `do_start`/`start`; must not deadlock if inflight never drains (bounded by
  `drain_timeout_secs`, then SIGTERM anyway — mirror `drain_and_stop`). Resolved via TEST-30.
- **ITEM-27** — verdict: PASS — additive `Draining` flag + a 503 gate on the transcribe entrypoint.
- **ITEM-28** — verdict: PASS — one shared `.no_proxy()` client; pure improvement/consistency.
- **ITEM-29** — verdict: PASS — additive CHECK constraint (safe, see migration note).
- **ITEM-30** — verdict: PASS — wires existing dead-coded `logs()`/`subscribe_logs()` to two routes.
- **ITEM-31** — verdict: PASS — additive snapshot routes; `snapshot_of` builder already exists.
- **ITEM-32** — verdict: PASS — additive read routes; un-deads existing fields.
- **ITEM-33** — verdict: CONCERN — adds a frontend logs viewer (a UI diff → needs e2e TEST-37 +
  `npm run check`/`gate:ui`). Resolved: mirror the llm-runtime logs UI + the existing voice card style.

**Do-not-regress guard:** ITEM-24/26 modify proven live paths; the drift loop (phase 5) + the blind
audit (phase 6) must re-check that pre-healthy-crash flap protection, the `is_failed()` gate, and
the idle-reaper drain are all still intact. Voice's mandatory-sha256 verification stays (not aligned
down to the llm runtime's weaker posture).

No `BLOCKED` verdicts. CONCERNs are all resolved in-plan (documented dispositions above); none require a plan amendment beyond what PLAN.md already states.
