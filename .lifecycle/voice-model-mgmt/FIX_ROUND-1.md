# FIX_ROUND-1 — voice-model-mgmt

Merged the Phase-6 blind-audit ledger (33 findings, 17 angles) and fixed every
confirmed HIGH + MEDIUM, plus the cheap LOWs. Then ran a fresh blind re-audit
round (2 agents: correctness/concurrency/resource-leak + security/api-contract).

## Fixed (round-1)
- **[HIGH security] SSRF-via-redirect** (`model.rs`) — arbitrary-URL downloads now go
  through `url_validator::validated_client_builder(PUBLIC_HTTP_OR_HTTPS)` (DNS-guard +
  per-hop redirect re-validation, 10-hop cap); trusted catalog keeps the plain client.
- **[HIGH security] Path traversal** (`model_handlers.rs`) — HF-repo on-disk filename is
  derived from the validated `name`; the remote filename is separately gated by
  `is_safe_remote_filename` (URL-only).
- **[HIGH perf] Catalog fetch-per-call + no cache** (`model_catalog.rs`) — added the
  promised short-TTL cache (5 min ok / 30 s err) keyed by source repo; invalidated on a
  source-repo change.
- **[MED concurrency] Drain double-clear race** (`auto_start.rs`) — `DRAINING` is now an
  `AtomicUsize` counter, so overlapping reaper + model-switch drains keep the front door
  shut until the LAST finishes.
- **[MED resource-leak] SSE hang after Complete** (`model_download_task.rs`/`model_handlers.rs`)
  — the terminal Complete payload is retained + replayed on a late subscribe, then the
  stream closes (no `rx.recv()` forever).
- **[MED resource-leak] Upload 5 GiB RAM buffering** (`model.rs`/`model_handlers.rs`) — the
  upload now streams to a temp file via `field.chunk()` with the cap enforced as bytes
  arrive (no whole-file buffering, no 2× clone).
- **[MED resource-leak] Download registry unbounded** (`model_download_task.rs`) — terminal
  tasks are pruned when the registry exceeds a cap.
- **[MED design] Config model-select overlap** (`VoiceConfigCard.tsx`) — the Model select is
  now populated from the INSTALLED library (a downloaded `large-v3` is selectable), not a
  hardcoded 4-list.
- **[MED patterns] F5 shared client** (`transcribe.rs`) — one shared `OnceLock` `.no_proxy()`
  inference client + per-request timeout.
- **[LOW security] URL creds in logs** (`model.rs`) — `redact_url` strips userinfo.
- **[LOW a11y] Upload label** (`UploadModelDrawer.tsx`) — `aria-label` on the Upload control.

## Rejected / accepted-as-is (with reason)
- `VoiceModelUpdate` refetch on `sync:voice_settings` (M) → RESOLVED via the TTL cache (the
  refetch now hits the cache, not upstream).
- `list_models` inline catalog fetch diverges from sibling (M) → cheap now (cached); kept for
  the `update_available` flag.
- Hand-rolled AddFromUrlForm, stale `#[allow(dead_code)]`, redundant refetch-on-complete (LOW)
  → cosmetic; not worth the churn/regression risk.

## Re-audit (blind, round-2) — NEW confirmed findings surfaced
The security re-audit confirmed the SSRF + traversal fixes sound (0 new). The correctness
re-audit found **4 NEW confirmed** issues the round-1 fixes exposed/left — carried into
FIX_ROUND-2:
1. [HIGH] library models can't actually run (`ensure_model` still gated on the 4-name list).
2. [MED] `.gguf` installed file orphaned (runtime resolved only `ggml-<name>.bin`).
3. [MED] upload temp leak if a later multipart field errors after the temp exists.
4. [LOW] `prune_terminal_tasks` holds a DashMap guard across `await` (stall hazard).

**New confirmed findings:** 4
