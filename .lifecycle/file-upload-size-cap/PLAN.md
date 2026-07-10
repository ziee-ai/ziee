# PLAN — configurable file-upload size cap (fix >50 MB → 400)

## Context
`POST /api/files/upload` rejects any file > 50 MB with HTTP 400 `FILE_TOO_LARGE`
because of a hardcoded `const MAX_FILE_SIZE = 50 * 1024 * 1024`
(`file/handlers/upload.rs:18`), even though the route body limit (200 MB) and
nginx (1 GB) allow far more. Make the per-file cap configurable (default 128 MB),
keep `nginx ≥ body limit ≥ handler cap`, and keep the frontend in sync via a
single shared constant. `.rds`/scientific binaries already pass the MIME check;
lock that in with tests.

## Items
- **ITEM-1**: Add `max_file_upload_mb: u64` (default 128) to `ServerConfig` in `core/config.rs` via a `#[serde(default = "default_max_file_upload_mb")]` field + `default_max_file_upload_mb() -> u64` free fn, mirroring `RateLimitConfig`'s `default_*` pattern.
- **ITEM-2**: Add a boot-set process-global to `core/app_state.rs`: `MAX_FILE_UPLOAD_BYTES: Lazy<Mutex<usize>>` (default 128 MiB) with poison-recovering `set_max_file_upload_bytes` / `get_max_file_upload_bytes`, plus `file_upload_body_limit_bytes() = get_max_file_upload_bytes() + 16 MiB`, mirroring `CACHES_CONFIG`.
- **ITEM-3**: Populate the global at boot from `config.server.max_file_upload_mb` in every boot path that builds the router — `main.rs` (near `set_caches_config`, before `build_api_router`) and `lib.rs` (before its router build) — leaving the 16 MB app-wide `DefaultBodyLimit` fallback untouched.
- **ITEM-4**: Replace `const FILE_UPLOAD_BODY_LIMIT` in `file/routes.rs` with `app_state::file_upload_body_limit_bytes()` at the `/files/upload` `DefaultBodyLimit::max(...)` layer.
- **ITEM-5**: Replace `const PROJECT_FILE_UPLOAD_BODY_LIMIT` in `file/project_extension/routes.rs` with `app_state::file_upload_body_limit_bytes()` at the `/projects/{id}/files/upload` layer.
- **ITEM-6**: In `file/handlers/upload.rs`, delete `MAX_FILE_SIZE`; read `get_max_file_upload_bytes()` in the size check and make the error message state the real limit in MiB (keep the `FILE_TOO_LARGE` code + 400).
- **ITEM-7**: Fix stale "100 MiB" size copy in the project-extension OpenAPI doc (`project_extension/handlers.rs`) and the `upload_file_docs` description so they describe the configurable/derived cap.
- **ITEM-8**: Add a shared frontend constant `MAX_FILE_UPLOAD_BYTES = 128 * 1024 * 1024` (+ a `128MB` label) in `src-app/ui/src/modules/file/constants.ts` and replace the 6 hardcoded `100 * 1024 * 1024` literals + "Maximum size is 100MB" messages across the chat- and project-extension components/stores with the shared constant + a label-driven message.
- **ITEM-9**: Plumb `ZIEE_MAX_FILE_UPLOAD_MB` through the docker web image: add `max_file_upload_mb: ${ZIEE_MAX_FILE_UPLOAD_MB}` under `server:` in `docker/web/config.template.yaml`, add the var to the `envsubst` allowlist in `docker/web/entrypoint.sh`, and add `ENV ZIEE_MAX_FILE_UPLOAD_MB=128` to `docker/web/Dockerfile`.
- **ITEM-10**: Tighten the `client_max_body_size` comment in `docker/web/nginx.conf` to document the `nginx ≥ body limit ≥ cap` invariant (value stays 1024m).
- **ITEM-11**: Add `max_file_upload_mb: Option<u64>` to `TestServerOptions` and interpolate it into the `server:` block of the harness YAML in `tests/common/harness_inner.rs`, mirroring `refresh_token_expiry_days`, so integration tests can spawn a server with a tiny cap.
- **ITEM-12**: Regression-guard (tests only, no code change) that scientific/binary data files pass the MIME sniff + smuggling check: gzip-framed `.rds` (`\x1f\x8b`) sniffs `application/gzip` and is accepted; an unknown-signature binary sniffs `None` and is accepted. Locks in the behavior verified during exploration so a future MIME-check tightening can't silently reject `.rds`/genomics uploads.

## Files to touch
- `src-app/server/src/core/config.rs`
- `src-app/server/src/core/app_state.rs`
- `src-app/server/src/main.rs`
- `src-app/server/src/lib.rs`
- `src-app/server/src/modules/file/routes.rs`
- `src-app/server/src/modules/file/project_extension/routes.rs`
- `src-app/server/src/modules/file/handlers/upload.rs`
- `src-app/server/src/modules/file/project_extension/handlers.rs`
- `src-app/server/src/modules/file/utils/magic.rs` (add tests only)
- `src-app/ui/src/modules/file/constants.ts` (new)
- `src-app/ui/src/modules/file/chat-extension/components/FileUploadButton.tsx`
- `src-app/ui/src/modules/file/chat-extension/components/FileUploadArea.tsx`
- `src-app/ui/src/modules/file/chat-extension/components/FilePasteHandler.tsx`
- `src-app/ui/src/modules/file/chat-extension/components/FileAttachMenuItem.tsx`
- `src-app/ui/src/modules/file/project-extension/components/ProjectFilesManagePanel.tsx`
- `src-app/ui/src/modules/file/project-extension/stores/ProjectFiles.store.ts`
- `docker/web/config.template.yaml`
- `docker/web/entrypoint.sh`
- `docker/web/Dockerfile`
- `docker/web/nginx.conf`
- `src-app/server/tests/common/harness_inner.rs`
- `src-app/server/tests/file/mod.rs` (tests)
- `src-app/server/tests/project/files_test.rs` (tests)
- `src-app/ui/tests/e2e/chat/file-upload-size-limit.spec.ts` (new test)

## Patterns to follow
- **Config field** → mirror `RateLimitConfig` + `default_rate_limit_*` in `core/config.rs` (`#[serde(default = "default_fn")]` + free fn).
- **Boot-set global read by both router-build and handler** → mirror `CACHES_CONFIG` / `SERVER_ADDR` in `core/app_state.rs` (`Lazy<Mutex<..>>` + poison-recovering `set_*`/`get_*`, set once in `main.rs`).
- **Per-route body limit** → the existing `DefaultBodyLimit::max(...)` layer already in `file/routes.rs` / `project_extension/routes.rs`; only swap the const for the getter.
- **Handler size check + `AppError::bad_request`** → the existing check in `upload_file_inner` (`file/handlers/upload.rs`); keep the exact error-code style.
- **Harness config override (Option numeric)** → mirror `refresh_token_expiry_days` field + `{refresh_days}` YAML interpolation in `tests/common/harness_inner.rs`.
- **Integration upload test** → mirror `test_upload_file_too_large` + the multipart pattern in `tests/file/mod.rs`, and the shared `upload_file` helper in `tests/project/helpers.rs`.
- **Frontend shared constant + `message.error` toast** → the existing `MAX_FILE_SIZE` consts + `message.error(...)` calls in the file chat-extension components (consolidate into one module-level `constants.ts`).
- **e2e composer-attach flow** → mirror `tests/e2e/chat/file-upload-error.spec.ts` (add-btn → filechooser → assert toast) for the client-side too-large rejection.
