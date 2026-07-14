# Chunk `ziee-file` — the generic multi-derivative blob STORE (CUT manifest)

Cut the **file STORE** (bytes + versions + CRUD-repository + storage backend +
upload-security validators + download-token/pagination DTOs + `files::*` perm
keys) out of ziee's `modules/file` into a new `ziee-file` SDK crate, leaving the
domain **PROCESSING / integration** (extraction engine, chat/project bridges,
LLM routing, RAG, deliverables, `available_files`, the HTTP handlers/routes) in
ziee behind a re-export shim. Two injected seams (`FileProcessor` / `FileEvents`)
sever the store's ingest path from the processing engine + sync/RAG. The one
domain leak in the store half — `files.workflow_run_id` — is removed entirely
and re-homed as a ziee-side `file_workflow_runs` join table (RESOLVED, N9).

## Design-gate — derivative STORAGE = SDK; derivative PRODUCTION = ziee

The `File`/`FileVersion` model carries processing-derivative counters
(`has_thumbnail`/`preview_page_count`/`text_page_count`/`processing_metadata`)
and `FileStorage` has geometry/text/image save-primitives. This does NOT make
them domain: the store is a *dumb multi-derivative blob store* — it persists
whatever derivative bytes it is handed and records their counts, but does not
know how to PRODUCE them. `ProcessingManager` (the producer) stays ziee; it is
reached from the store's ingest only through the `FileProcessor` seam. The
`ProcessingResult`/`ProcessingMetadata` *shapes* (pure data) move to the SDK.

## Design-gate — OpenAPI byte-identity (N7 / STOP-risk #3)

The store's `File.*` aide operations must stay byte-identical or `types_ts_parity`
fails. To eliminate this risk the HTTP `handlers/**` + `routes.rs` (`file_router()`
+ every `*_docs`) are KEPT IN ziee verbatim (they reference the moved store
symbols via the shim). The aide route builder therefore runs the exact same
registrations in the exact same order. **Verified:** golden regen → `types.ui.ts`
/ `types.desktop.ts` BYTE-IDENTICAL; `openapi.{ui,desktop}.json` CANONICALLY-EQUAL.

## Design-gate — `workflow_run_id` → ziee side-table (N9, RESOLVED)

The SDK `files` table has NO run column. ziee's workflow module owns a
`file_workflow_runs(file_id, workflow_run_id)` join table (FK + `ON DELETE
CASCADE` both sides). The former `FileRepository::{set_workflow_run_id,
list_ids_by_workflow_run}` become ziee-side `Repos.file_workflow_runs.{link,
list_file_ids}`. The store is fully domain-agnostic (no `workflow_runs`
reference anywhere in `ziee-file`).

## Files

- move: `server/src/modules/file/models.rs` → `sdk/crates/ziee-file/src/models.rs` (+ `ProcessingResult` folded in from `processing/mod.rs`)
- move: `server/src/modules/file/types.rs` → `sdk/crates/ziee-file/src/types.rs`
- move: `server/src/modules/file/repository.rs` → `sdk/crates/ziee-file/src/repository.rs`
- move: `server/src/modules/file/permissions.rs` → `sdk/crates/ziee-file/src/permissions.rs`
- move: `server/src/modules/file/storage/mod.rs` → `sdk/crates/ziee-file/src/storage/mod.rs`
- move: `server/src/modules/file/storage/filesystem.rs` → `sdk/crates/ziee-file/src/storage/filesystem.rs`
- move: `server/src/modules/file/storage/manager.rs` → `sdk/crates/ziee-file/src/storage/manager.rs`
- move: `server/src/modules/file/utils/magic.rs` → `sdk/crates/ziee-file/src/utils/magic.rs`
- move: `server/src/modules/file/utils/zipbomb.rs` → `sdk/crates/ziee-file/src/utils/zipbomb.rs`
- move: `server/src/modules/file/utils/mod.rs`(`extension_of`) → `sdk/crates/ziee-file/src/utils/mod.rs`
- move: `server/src/modules/file/migrations/202607140125_file_schema.sql` → `sdk/crates/ziee-file/migrations/202607140125_file_schema.sql` (`workflow_run_id` col + index dropped)
- new: `sdk/crates/ziee-file/src/seams.rs` — `FileProcessor` / `FileEvents`
- new: `sdk/crates/ziee-file/src/ingest.rs` — `store_processed` + `ingest_bytes` (store half, seam-driven)
- new: `sdk/crates/ziee-file/src/lib.rs`, `Cargo.toml`, `build.rs` (file-only build DB, mirrors ziee-auth)

## Symbols

- symbol: `File` (models.rs)
- symbol: `FileVersion` (models.rs)
- symbol: `FileCreateData` (models.rs)
- symbol: `FileVersionCreateData` (models.rs)
- symbol: `ProcessingMetadata` (models.rs)
- symbol: `ProcessingResult` (models.rs)
- symbol: `FileRepository` (repository.rs)
- symbol: `FileStorage` (storage/mod.rs)
- symbol: `FilesystemStorage` (storage/filesystem.rs)
- symbol: `get_file_storage` (storage/manager.rs)
- symbol: `init_file_storage` (storage/manager.rs)
- symbol: `FileListResponse` (types.rs)
- symbol: `DownloadTokenClaims` (types.rs)
- symbol: `DOWNLOAD_TOKEN_AUDIENCE` (types.rs)
- symbol: `extension_of` (utils/mod.rs)
- symbol: `FilesRead` (permissions.rs)
- symbol: `FilesUpload` (permissions.rs)
- symbol: `FilesDownload` (permissions.rs)
- symbol: `FilesDelete` (permissions.rs)
- symbol: `FilesPreview` (permissions.rs)
- symbol: `FilesGenerateToken` (permissions.rs)
- symbol: `FileProcessor` (seams.rs)
- symbol: `FileEvents` (seams.rs)
- symbol: `store_processed` (ingest.rs)

## Stays app-side (ziee `modules::file` residue + shim)

`processing/**`, `utils/{pandoc,pdfium,spreadsheet,export,embedded}`,
`geometry_backfill.rs`, `available_files.rs`, `provider_routing.rs`,
`deliverables.rs`, `chat_extension/**`, `project_extension/**`,
`handlers/**`, `routes.rs`, `sync.rs`, `config.rs`, `ingest.rs`
(orchestration wrapper + the `FileProcessor`/`FileEvents` impls),
`versioning.rs`. The `files_fkeys` (users/self FKs) + `grant_permissions`
migrations stay ziee; the new `file_workflow_runs` migration + `file_runs.rs`
repo are ziee (workflow module). `mod.rs` re-exports the moved store symbols
(`pub use ziee_file::{models, permissions, storage, types}` + `FileRepository`
+ `init_file_storage`; `utils` re-exports `extension_of/magic/zipbomb`;
`processing` re-exports `ProcessingResult`) so all ~59 store consumers compile
unchanged.
