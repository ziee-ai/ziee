# DECISIONS — resolved up front

### DEC-1: Configurable cap or a fixed raised constant?
**Resolution:** Configurable via YAML (`server.max_file_upload_mb`) + the docker env var, with a code default.
**Basis:** user — chosen in plan approval ("Configurable + default").

### DEC-2: What is the default cap value?
**Resolution:** 128 (MiB).
**Basis:** user — set to 128 during planning.

### DEC-3: How is the route body limit derived from the cap?
**Resolution:** `file_upload_body_limit_bytes() = cap + 16 MiB` slack, applied to both upload routes.
**Basis:** convention — mirrors the existing headroom philosophy (route body limit was already > the handler cap to cover multipart framing/extra fields); 16 MiB is ample for a single-file multipart body.

### DEC-4: Env var name for the docker layer?
**Resolution:** `ZIEE_MAX_FILE_UPLOAD_MB`.
**Basis:** convention — matches the `ZIEE_*` naming of every other var in `docker/web/entrypoint.sh` / `config.template.yaml`.

### DEC-5: Config key name and location?
**Resolution:** `server.max_file_upload_mb: u64`, a new field on `ServerConfig`.
**Basis:** codebase — the per-route `DefaultBodyLimit` and the server bind live under the `server` config domain; adjacent to `rate_limit`/`cors`.

### DEC-6: Units — is `_mb` decimal (10^6) or binary (2^20)?
**Resolution:** binary — `max_file_upload_mb * 1024 * 1024`, computed with `saturating_mul` to avoid overflow on absurd values.
**Basis:** codebase — the existing `MAX_FILE_SIZE`/body-limit consts all use `N * 1024 * 1024`; stay consistent.

### DEC-7: Backend error message wording when over the cap?
**Resolution:** keep code `FILE_TOO_LARGE` (400); message: `File size exceeds the maximum of {cap_mib} MiB`.
**Basis:** codebase — preserves the existing error-code contract; states the real (configured) limit per the task requirement.

### DEC-8: Frontend sync mechanism + value?
**Resolution:** single shared constant `MAX_FILE_UPLOAD_BYTES = 128 * 1024 * 1024` in `modules/file/constants.ts`; no new API endpoint. Client message keeps the existing style: `Maximum size is 128MB`.
**Basis:** user — chose "Shared constant" over exposing the cap via API.

### DEC-9: Keep nginx `client_max_body_size` at 1024m?
**Resolution:** Yes — unchanged (comment tightened to document the invariant). 1024m ≥ the derived body limit for any cap up to ~1000 MiB.
**Basis:** user — confirmed nginx stays at 1 GB during planning.

### DEC-10: Change the 10 GiB per-user storage quota?
**Resolution:** No — out of scope; `PER_USER_STORAGE_QUOTA_BYTES` unchanged.
**Basis:** convention — the task targets the per-file cap only; quota is an orthogonal limit.

### DEC-11: Raise the app-wide 16 MB `DefaultBodyLimit` fallback (main.rs/lib.rs)?
**Resolution:** No — the upload routes attach their own per-route `DefaultBodyLimit` that overrides the app-wide fallback for those paths.
**Basis:** codebase — axum's per-route layer wins over the global default; other routes keep the tight 16 MB safety net.

### DEC-12: How does the e2e create an oversize file without a huge alloc/transfer?
**Resolution:** create a sparse file (`ftruncate` to 128 MiB + 1) on disk and pass its path to `setInputFiles`; the browser reads `File.size` from metadata and rejects client-side, so no bytes are written to disk blocks or sent over the wire.
**Basis:** convention — standard Playwright large-file handling; the client check only reads `.size`.

### DEC-13: What happens to the existing 101 MB `test_upload_file_too_large`?
**Resolution:** repurpose it onto the small per-test cap (`max_file_upload_mb: Some(1)`) with a ~1.5 MiB body; drop the 101 MB allocation and the stale "100MB" comment.
**Basis:** codebase — with the default now 128 MiB, a 101 MB upload would pass; the test must assert the boundary against a configured cap.
