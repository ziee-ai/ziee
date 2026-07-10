# TESTS — every ITEM mapped; boundary tests use a tiny per-test cap (no huge allocs)

Strategy: ITEM-11 gives the harness a `max_file_upload_mb` knob, so integration
boundary tests spawn a server with a **1 MiB** cap and exercise accept/reject with
KB-to-low-MB files. The `.rds`/MIME behavior is locked at both unit and HTTP tiers.
The UI cap change is proven by an e2e client-side rejection spec (the oversize file
is created **sparse** on disk — the browser reads `File.size` from metadata, so no
128 MB is ever written or transferred).

## Tests
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/core/config.rs` — asserts: `default_max_file_upload_mb()` returns 128, a YAML omitting the key deserializes to 128, and an explicit `max_file_upload_mb: 256` deserializes to 256.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/core/app_state.rs` — asserts: `set_max_file_upload_bytes`/`get_max_file_upload_bytes` round-trip, and `file_upload_body_limit_bytes()` == cap + 16 MiB.
- **TEST-3** (tier: integration) [covers: ITEM-3, ITEM-4, ITEM-6, ITEM-11] file: `src-app/server/tests/file/mod.rs` — asserts: with `TestServerOptions{ max_file_upload_mb: Some(1) }`, a ~0.9 MiB upload to `/files/upload` → 201; a ~1.5 MiB upload → 400 `FILE_TOO_LARGE` with a limit-stating message; and an ~18 MiB upload (above the derived body limit cap+16 MiB) → 413 from the `DefaultBodyLimit` layer (proves the route body limit is derived from the configured cap, not a constant — config→global→body-limit layer→handler end-to-end).
- **TEST-4** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/project/files_test.rs` — asserts: with `max_file_upload_mb: Some(1)`, `/projects/{id}/files/upload` accepts a ~0.9 MiB file (201, attached) and rejects a ~1.5 MiB file (400 `FILE_TOO_LARGE`).
- **TEST-5** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/file/mod.rs` — asserts: a gzip-magic (`\x1f\x8b…`) `.rds`-style binary under the cap uploads → 201 and its stored `mime_type` is `application/gzip` (no `MIME_MISMATCH`).
- **TEST-6** (tier: unit) [covers: ITEM-12] file: `src-app/server/src/modules/file/utils/magic.rs` — asserts: `sniff_mime` on gzip magic → `application/gzip` and `smuggling_rejection(Some("application/gzip"), "application/octet-stream")` → `None`; an unknown-signature binary → `sniff_mime` `None` and `smuggling_rejection(None, …)` `None`.
- **TEST-7** (tier: unit) [covers: ITEM-7] file: `src-app/server/src/modules/file/project_extension/handlers.rs` — asserts: the extracted upload-and-attach description const contains no stale `100 MiB`/`over 100 MiB` copy and describes the configurable per-file size cap.
- **TEST-8** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/file-upload-size-limit.spec.ts` — asserts: attaching an oversize (>128 MiB, sparse-on-disk) file via the chat composer surfaces a "128MB" too-large error toast and fires no `POST /api/files/upload`.
- **TEST-9** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/core/app_state.rs` — asserts: `docker/web/entrypoint.sh` envsubst allowlist AND `docker/web/config.template.yaml` both contain `ZIEE_MAX_FILE_UPLOAD_MB`/`max_file_upload_mb` (guards the classic "added the template key but forgot the allowlist → literal `${...}` renders" bug).
- **TEST-10** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/core/app_state.rs` — asserts: the `client_max_body_size` in `docker/web/nginx.conf`, parsed to bytes, is ≥ `file_upload_body_limit_bytes()` at the default cap (the `nginx ≥ body limit ≥ cap` invariant).
