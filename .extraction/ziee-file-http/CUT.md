# Chunk `ziee-file-http` — the mountable file HTTP surface (CUT manifest)

Complete the file extraction the `ziee-file` chunk deliberately deferred (to
preserve OpenAPI byte-identity): move the **store-generic** file HTTP handler
layer out of ziee's `modules/file/{handlers,routes}` into a mountable,
resolver-generic **`file_routes::<R>()`** bundle in `ziee-file`, mirroring
`ziee-auth`'s `auth_routes`. A second app now mounts working file endpoints
(list/get/preview/raw/thumbnail/text/text-rects/delete, download +
download-token, version reads + restore) instead of wiring its own. The file
**permission constants** already moved with the store (chunk `ziee-file`), so no
perm move is needed here. Equivalence-preserving MOVE: byte strings, error
shapes, response types, and — the hard gate — the OpenAPI spec are byte-identical
on BOTH surfaces.

## Design-gate — MOVE only the store-generic handlers; PROCESSING/identity stays ziee

The store handlers split cleanly along a coupling seam:

- **Store-generic (MOVED)** — read/serve/delete over the `FileRepository` +
  `FileStorage` + the injected `FileEvents` seam + the download-token signer.
  Nothing here names `ProcessingManager`, pandoc, `file_rag`, `versioning`, or
  the identity re-check.
- **Processing/domain (STAYED ziee-side)** — see the "Stays ziee-side" section.

The moved handlers are generic over the app's injected
`ziee_framework::permissions::IdentityResolver`, **fixed to this crate's
`ziee_auth::{User, Group}` wire types** (every file response is owner-scoped by
`user.id`, and the store is keyed by a `Uuid` user id). ziee mounts with
`ZieeIdentityResolver`; a second app mounts with its own resolver. This mirrors
`ziee-auth`'s decision exactly (the SDK's identity wire type IS ziee-auth's
User/Group).

## Design-gate — OpenAPI byte-identity (THE gate — why it was deferred)

The `File.*` aide operations cross a crate boundary. Held byte-identical by the
same four mechanisms `ziee-auth-routes` proved:
- **schema DTOs** (`File`, `FileVersion`, `FileListResponse`, the download-token
  DTOs, `BlobType`, `TextRectsResponse`/`HighlightRect`, the version request/query
  bodies) already live in `ziee-file` (chunk `ziee-file`) or moved verbatim with
  the handlers — schemars keys by SHORT ident, so every `$ref`/schema NAME is
  byte-stable.
- **`with_permission::<Perms>`** emits each perm type's `NAME`/`PERMISSION`/
  `DESCRIPTION` CONST strings (the `files::*` keys, unchanged) — 403 schema stable.
- **`RequirePermissions<R, Perms>`** derives `OperationIo` with no request-body
  schema → its OpenAPI contribution is empty and R-independent; making handlers
  generic over `R` cannot move the spec.
- **operationIds** (`File.list`, `File.download`, `File.restore`, …) are string
  literals in the moved `*_docs` fns, carried verbatim.
- **order-independence**: `emit_ts.rs` sorts endpoints + schemas; the golden
  compares `openapi.json` canonically (`jq -S`). So splitting the router across
  two crates (incl. the GET/POST split on `/files/{file_id}/versions` and the
  GET/DELETE on `/files/{file_id}`) does not move the spec — axum merges the
  method routers on the shared path.

**SPIKE result (before commit):** regen ui + desktop → `types.ui.ts`
BYTE-IDENTICAL, `types.desktop.ts` BYTE-IDENTICAL, `openapi.ui.json`
CANONICALLY-EQUAL, `openapi.desktop.json` CANONICALLY-EQUAL vs
`.extraction/baseline`. Generated files `git checkout`-restored after. Gate GREEN.

## Files

### New (SDK — `ziee-file`, feature-gated `routes`, default-on)
- new: `sdk/crates/ziee-file/src/http/mod.rs` — the routes-bundle module.
- new: `sdk/crates/ziee-file/src/http/context.rs` — `FileContext` + `DownloadTokenSigner`.
- new: `sdk/crates/ziee-file/src/http/routes.rs` — `file_routes::<R>()`.
- new: `sdk/crates/ziee-file/src/http/handlers/mod.rs`.
- move: ziee `handlers/management.rs` (574 L, in full) → `http/handlers/management.rs`.
- move (partial): ziee `handlers/download.rs`'s `download_file` +
  `generate_download_token` + `content_disposition` + the two cache consts →
  `http/handlers/download.rs`.
- move (partial): ziee `handlers/versions.rs`'s `list_versions` /
  `get_head_version` / `get_version` / `restore_version` / `download_version` /
  `preview_version` / `text_version` + `version_and_file` + the version request/
  query DTOs → `http/handlers/versions.rs`.

### Edited (SDK)
- edit: `sdk/crates/ziee-file/Cargo.toml` — add `[features] default=["routes"]`,
  `routes=["dep:aide","dep:axum","dep:ziee-framework","dep:ziee-auth","dep:jsonwebtoken"]`
  + optional deps (aide gains `axum-query` for the query extractors' `OperationInput`).
- edit: `sdk/crates/ziee-file/src/lib.rs` — `#[cfg(feature="routes")] pub mod http;`.
- edit: `sdk/Cargo.lock` — aide/axum/jsonwebtoken now reach ziee-file.

### App-side → THIN CONSUMER (equivalence-preserving)
- edit: `src-app/server/src/modules/file/routes.rs` — `file_router()` now
  `ziee_file::http::file_routes::<ZieeIdentityResolver>()` merged with the
  RETAINED routes (upload / export / download-with-token / version-append POST /
  deliverables). Same paths, same operationIds.
- edit: `src-app/server/src/modules/file/handlers/download.rs` — keep ONLY
  `download_with_token` (+ docs); re-export `content_disposition` from the SDK
  (external consumers: chat export); import `FILE_CONTENT_CACHE_CONTROL`.
- delete: `src-app/server/src/modules/file/handlers/management.rs` (fully moved).
- edit: `src-app/server/src/modules/file/handlers/versions.rs` — keep ONLY
  `append_version` (+ `AppendVersionRequest` + docs).
- edit: `src-app/server/src/modules/file/handlers/mod.rs` — drop `management`.
- edit: `src-app/server/src/modules/file/ingest.rs` — extend `ZieeFileEvents`
  with `on_committed` (→ `file_rag::spawn_reindex`, the restore RAG hook) + add
  `build_file_context(pool, jwt)`.
- edit: `src-app/server/src/lib.rs` + `src-app/server/src/main.rs` — layer
  `Extension(build_file_context(pool, &config.jwt))` at BOTH server-setup sites
  (the bin's `main.rs` block + the shared/desktop `setup_server` in `lib.rs`).

## Symbols moved into `ziee_file::http`
- handlers (15) + their `*_docs`: `list_files`, `get_file`, `get_preview`,
  `get_raw`, `get_thumbnail`, `get_text_content`, `get_text_rects`, `delete_file`,
  `download_file`, `generate_download_token`, `list_versions`, `get_head_version`,
  `get_version`, `restore_version`, `download_version`, `preview_version`,
  `text_version`.
- helper/consts: `content_disposition` (now `pub`), `FILE_CONTENT_CACHE_CONTROL`,
  `FILE_HEAD_CACHE_CONTROL`, `version_and_file`, `align_span_to_boxes` (+ tests).
- DTOs: `TextRectsQuery`, `HighlightRect`, `TextRectsResponse`,
  `RestoreVersionRequest`, `ListVersionsQuery`.
- route builder: `file_routes<R>`.
- new SDK types: `FileContext`, `DownloadTokenSigner`.

## Stays ziee-side (processing / domain / identity — NOT the reusable surface)
- `upload_file` (`ProcessingManager` producer + per-user quota + `file_rag`
  index) — the biggest processing coupling.
- `export_file` (pandoc render).
- `append_version` (`versioning::commit_new_version` → `ProcessingManager`).
- `download_with_token` — re-verifies identity BY user-id from the signed claim
  (`Repos.user.get_by_id` + `is_active` + `check_permission_union`), which does
  not fit the request-`Parts`-based `IdentityResolver` seam cleanly. **Reported
  snag #1; left ziee-side per the chunk's explicit allowance.** The rest still
  moved.
- the conversation `deliverables` routes (chat/domain surface — `conversations/*`).
- `ZieeFileProcessor`/`ZieeFileEvents` seam impls, `build_file_context`, the
  file-module JWT config, `sync.rs`, `versioning.rs`, `provider_routing.rs`,
  `available_files.rs`, `geometry_backfill.rs`, `processing/**`.
