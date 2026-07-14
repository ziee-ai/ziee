# Chunk `ziee-file` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form. Files not
listed moved BYTE-FOR-BYTE: `storage/manager.rs`, `utils/magic.rs`,
`utils/zipbomb.rs` (0 changed lines vs the pre-move blob).

- **T-1** `FileStorage` / `FilesystemStorage` (`storage/{mod,filesystem}.rs`):
  `use crate::common::AppError;` → `use ziee_core::AppError;` (1 line each).
  **why:** the store's error type is the SDK-shared `ziee_core::AppError` (ziee's
  `crate::common::AppError` is a re-export of it — same type, so the seam +
  every consumer typecheck unchanged).

- **T-2** `File` model file (`models.rs`): `ProcessingResult` struct ADDED
  (folded in verbatim from `processing/mod.rs`). **why:** `ProcessingResult` is
  pure data (the store's persist input) and belongs with `ProcessingMetadata` in
  the store; the PRODUCER (`ProcessingManager`) stays ziee and re-exports the
  type via `processing::mod`. No existing field changed.

- **T-3** `FileListResponse` etc. (`types.rs`):
  `use crate::modules::file::models::File;` → `use crate::models::File;`.
  **why:** intra-crate path after the move. Schemars keys preserved → OpenAPI
  schema names byte-identical.

- **T-4** `FilesRead`/`FilesUpload`/… (`permissions.rs`):
  `use crate::modules::permissions::types::PermissionCheck;` →
  `use ziee_identity::PermissionCheck;`. **why:** the permission-key seam
  (ziee's `permissions::types::PermissionCheck` is a re-export of
  `ziee_identity::PermissionCheck` — same trait). Permission STRINGS unchanged
  (`files::read`…); the Users-group grant migration stays ziee (N9).

- **T-5** `FileRepository` (`repository.rs`): (a) `AppError` + `models` path
  swaps as T-1/T-3; (b) `crate::common::PAGINATION_MAX_PER_PAGE as i64` → inline
  `100i64`; (c) REMOVED `set_workflow_run_id` + `list_ids_by_workflow_run` (+
  a NOTE). **why:** (b) trivially inlines the one shared const so the repo has
  zero `crate::common` reach; (c) the file↔run link is domain — re-homed to
  ziee's `file_workflow_runs` join table (N9, RESOLVED). No SQL query text on
  any RETAINED method changed → row shapes + `query!` verification identical.

- **T-6** `202607140125_file_schema.sql`: DROPPED the `workflow_run_id uuid`
  column + `idx_files_workflow_run_id`. **why:** the store carries no run
  column (N9). The relationship is preserved by the new ziee-side
  `file_workflow_runs` table (workflow migration `202607144231`). The base
  filename/version is preserved so the merged `_sqlx_migrations` history keeps
  its slot; the FK-less `files`/`file_versions` subset provisions the SDK's
  file-only build DB standalone.

- **T-7** ziee `modules/file/ingest.rs` (RESIDUE, not a move): rewritten to
  delegate to `ziee_file::ingest::ingest_bytes(&Repos.file, &ZieeFileProcessor,
  &ZieeFileEvents, …)` then INSERT a `file_workflow_runs` join row when
  `workflow_run_id` is `Some`. The 7-arg public signature is preserved →
  `workflow/runner.rs` + `mcp/resource_link.rs` callers unchanged. **why:** the
  STORE half moved; ziee keeps only the seam impls + the domain run-linkage.

- **T-8** call-site rewires (RESIDUE): `Repos.file.set_workflow_run_id(f,r)` →
  `Repos.file_workflow_runs.link(f,r)` (`mcp/resource_link.rs`);
  `Repos.file.list_ids_by_workflow_run(r)` →
  `Repos.file_workflow_runs.list_file_ids(r)` (`workflow/handlers/mod.rs` ×2,
  `scheduler/continue_chat.rs`). **why:** the two helpers moved off the store
  repo onto the ziee-side join-table repo.

## Decision — where the HTTP handlers/routes live

The store `handlers/**` + `routes.rs` are pervasively coupled to the global
`Repos` aggregator, `get_max_file_upload_bytes`, and (for `download_with_token`)
the identity re-check (`Repos.user`/`Repos.group`/`check_permission_union`).
Threading a `FileContext` through ~1700 LOC of handlers is mechanical but is the
single largest source of OpenAPI byte-identity risk (STOP-risk #3) for near-zero
reusable-store value (they are HTTP glue, not the reusable engine).

**Resolution:** KEEP `handlers/**` + `routes.rs` in ziee, referencing the moved
store symbols through the `modules::file` shim. This guarantees byte-identical
OpenAPI (the aide builder is unchanged) while the reusable STORE ENGINE
(models/storage/repository/types/utils/permissions/seams/store-ingest) cleanly
extracts. The `FileProcessor`/`FileEvents` seams are exercised by the SDK's
`ingest`/`store_processed` path (workflow-run artifacts, MCP `resource_link`,
chat tool-result saves — all routed through ziee's `ingest_bytes` delegate).
The full handler-layer move is a tracked follow-up sub-chunk (`ziee-file-http`),
gated on a `FileContext` seam + the download-token identity resolver.

No `TBD` / `TODO` / `ASK` / `???` remain.
