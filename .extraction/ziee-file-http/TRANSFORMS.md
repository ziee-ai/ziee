# Chunk `ziee-file-http` — TRANSFORMS (design-gate resolutions, zero TBD)

Every symbol whose SDK form differs from its pre-move ziee form, plus the two
snag Decisions the chunk called out.

## T-1 — the moved handlers become generic over `R` + pull `Extension<FileContext>`
Each moved permission-gated handler gains
`<R: IdentityResolver<User = User, Group = Group>>` (`User`/`Group` =
`ziee_auth::{User, Group}`), its extractor `RequirePermissions<(P,)>` →
`RequirePermissions<R, (P,)>`, and its global reaches are replaced by the
injected `FileContext`:
- `Repos.file.<m>()` → `ctx.files.<m>()` (`ctx: Extension<FileContext>`;
  `ctx.files: Arc<FileRepository>`).
- `crate::modules::file::sync::publish_file_deleted_with_origin(...)` →
  `ctx.events.on_file_deleted(...)` (`delete_file`).
- `crate::modules::file::sync::publish_file_changed_with_origin(...)` +
  `crate::modules::file_rag::ingest::spawn_reindex(...)` →
  `ctx.events.on_file_changed(...)` + `ctx.events.on_committed(user_id, file_id,
  false)` (`restore_version`).
- `get_jwt_config()` (issuer/secret) → `ctx.download_token.{issuer,secret}`
  (`generate_download_token`).
**why:** severs the store surface from ziee's `Repos`/`sync`/`file_rag`/JWT
globals via one per-request handle, exactly as `ziee-auth`'s `AuthContext`. No
handler names a ziee-domain symbol. `get_file_storage()` stays a store global
(moved in chunk `ziee-file`), so it is called unchanged. Behaviour is
byte-identical (the `FileEvents` seam impls forward to the same
`sync::publish`/`spawn_reindex` functions).

## T-2 — import-path rewrites (mechanical)
`crate::common::{ApiResult, AppError}` → `ziee_core::{ApiResult, AppError}`;
`crate::modules::permissions::{extractors::RequirePermissions, openapi::with_permission}`
→ `ziee_framework::permissions::{RequirePermissions, with_permission,
IdentityResolver}`; `crate::modules::sync::SyncOrigin` →
`ziee_framework::sync::SyncOrigin`; `crate::modules::file::{models,types,
permissions,storage::manager::get_file_storage}` → intra-crate `crate::{models,
types,permissions,get_file_storage}`. **why:** post-move paths; every target is
the SAME type ziee re-exports (same trait/type identity → consumers + OpenAPI
unchanged).

## T-3 — `content_disposition` `pub(crate)` → `pub`
**why:** it moves to the SDK and is consumed cross-crate by ziee's retained
`download_with_token` + `export_file` + `chat::core::export` (all via the
`handlers::download::content_disposition` re-export shim), so it must be public.
Body byte-identical.

## T-4 — `list_versions` inlines the two pagination consts
`crate::common::{DEFAULT_PAGE_SIZE, PAGINATION_MAX_PER_PAGE} as i64` (both = 100)
→ crate-local `const VERSIONS_DEFAULT_LIMIT/VERSIONS_MAX_LIMIT: i64 = 100`.
**why:** keeps the store surface free of `crate::common` reach, exactly as chunk
`ziee-file` inlined the same `PAGINATION_MAX_PER_PAGE` in the repository (T-5
there). Numerically identical → same clamp behaviour, same wire output.

## T-5 — `version_and_file` takes `files: &FileRepository`
The private helper was `Repos.file`-global; it now takes the repo by ref from the
handler's `ctx.files`. **why:** the store surface has no `Repos`; the helper is
called only from the three moved pinned-version handlers, each passing
`&ctx.files`. Query text unchanged.

## T-6 — ziee `ZieeFileEvents` gains `on_committed`
The existing seam impl (chunk `ziee-file`) implemented only `on_file_changed`/
`on_file_deleted`; `on_committed` was the default no-op. It now routes
`is_new == false` → `file_rag::ingest::spawn_reindex`. **why:** the moved
`restore_version` re-indexes the new head; the seam already reserved
`on_committed(is_new)` for exactly this. `is_new == true` (brand-new-file
`spawn_index`, which needs the full `&File`) is never reached through this seam —
it stays on the still-ziee upload path.

## T-7 — `file_router()` becomes a merge of the SDK bundle + retained routes (RESIDUE)
`ziee_file::http::file_routes::<ZieeIdentityResolver>()` merged with the 6
retained routes (upload+body-limit / export / download-with-token /
version-append POST / 2× deliverables). The GET `/files/{file_id}/versions` (SDK)
+ POST (ziee) merge on the shared path; likewise GET+DELETE `/files/{file_id}`.
**why:** same registrations, same order-independent spec. **Verified:** the
regenerated `/api/files/{file_id}/versions` carries `{get, post}` and
`/api/files/{file_id}` carries `{get, delete}`, golden byte-identical.

## T-8 — `build_file_context` + the Extension layer at BOTH server-setup sites (RESIDUE)
`modules::file::ingest::build_file_context(pool, &config.jwt)` builds the
`FileContext` (repo + `ZieeFileEvents` + signer from `config.jwt`), layered as an
axum `Extension` in BOTH `main.rs` (the `ziee` binary's own router block) and
`lib.rs::setup_server` (the shared path the DESKTOP embed + openapi-gen use).
**why:** `main.rs` re-declares its own module tree and builds its own router, so
the layer is required in both or the file routes 500 at runtime on a missing
extension. Confirmed: adding the `main.rs` layer cleared the bin's
`build_file_context is never used` dead-code warning.

---

## Decision — snag #1: the download-token identity re-check (`download_with_token`)
`download_with_token` verifies a signed file-download token, extracts the
`user_id` claim, then RE-RESOLVES that identity by id (`Repos.user.get_by_id`,
`is_active`, `get_user_groups`, `check_permission_union("files::download")`) — a
revocation re-check (audit 05-file F-06). The `IdentityResolver` seam
authenticates from request `Parts` (a bearer JWT), not by a `user_id` claim, and
has no by-id re-resolution method.

**Resolution (per the chunk's explicit allowance):** KEEP `download_with_token`
ziee-side. Making it resolver-generic would require expanding the framework
`IdentityResolver` with a by-id re-verify method — out of scope and higher-risk
than the value (one unauthenticated handler). It keeps using ziee's `Repos` +
`check_permission_union` unchanged; it re-imports the moved `content_disposition`
+ `FILE_CONTENT_CACHE_CONTROL` from the SDK. Everything else in `download.rs`
(the authenticated `download_file` + `generate_download_token`) moved. **No STOP
condition hit** (the gate said this handler MAY stay ziee-side).

## Decision — snag #2: the PROCESSING-coupled handlers
`upload_file` (the `ProcessingManager` producer + per-user quota + `file_rag`
index), `export_file` (pandoc), and `append_version` (via
`versioning::commit_new_version` → `ProcessingManager`) all need the processing
engine.

**Resolution:** KEEP them ziee-side. They are NOT store-generic — the store
persists derivative bytes but does not PRODUCE them (the `ziee-file` design-gate).
Dragging `ProcessingManager`/pandoc into the SDK would violate that gate; routing
them through the existing `FileProcessor` seam would be a genuine refactor of
`commit_new_version` (behaviour-drift risk) for little reusable value. The
already-defined `FileProcessor`/`FileEvents` seams remain the store's ingest
injection; these HTTP handlers stay on ziee's `ingest`/`versioning` orchestration.
The conversation `deliverables` routes are a chat/domain surface and likewise stay.

No `TBD` / `TODO` / `ASK` / `???` remain.
