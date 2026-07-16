# PLAN — code_sandbox read tools cannot read tool-produced artifacts

## Context

The code-sandbox `read_file` tool fails on model-authored artifacts (files a tool
produced, `created_by IN ('mcp','llm')`) with an opaque
`{"code":-32603,"message":"tool read_file failed"}`. Two independent defects:

1. `load_file_content` (`code_sandbox/tools/files.rs:161`) resolves a filename via
   (a) the per-conversation workspace, then (b) a fallback over `ctx.files`
   (`f.filename == filename && f.user_id == ctx.user_id`). `ctx.files` comes from
   `code_sandbox/repository.rs::get_conversation_files`, which unions only
   attachment + project files — never `created_by IN ('mcp','llm')`. So artifacts
   are unresolvable by the read tools and unlisted by `list_files`.
2. The MCP `dispatch` (`code_sandbox/handlers.rs:265-278`) flattens every tool
   error to `JsonRpcError::internal("tool {name} failed")` (-32603), discarding the
   real `AppError`; and the genuine not-found at `files.rs:234` is a 500 that
   embeds the host path.

This mirrors PR #152 (`files-mcp-model-authored-scope`), which taught the files-MCP
resolver about `created_by IN ('mcp','llm')`. Scope (khoi-approved): fix the two
read tools (`read_file`/`edit_file`, both via `load_file_content`) AND `list_files`,
reusing the SAME `model_authored_file_ids` source; do NOT change
`get_conversation_files` or the bwrap bind-mount (artifacts stay not `cat`-able in
the `execute_command` shell, by design).

## Items

- **ITEM-1**: `load_file_content` (`code_sandbox/tools/files.rs`) gains a model-authored
  fallback — after the workspace miss AND the `ctx.files` filename/user match both
  fail, resolve model-authored artifacts for the conversation via
  `file::available_files::model_authored_file_ids(conversation_id, user_id)` +
  `Repos.file.get_by_ids_and_user`, filter by filename, apply the same
  `>1 → AMBIGUOUS_FILENAME` rule as the attachment branch, and on a single match
  load bytes via `extension_of` + `get_file_storage().load_original(user_id,
  blob_version_id, ext)` + `String::from_utf8`. Fixes `read_file` + `edit_file`
  (both route through `load_file_content`). Workspace-first + attachment-first
  ordering preserved (the fallback runs only when both miss).
- **ITEM-2**: the genuine not-found at `files.rs` (currently
  `io_err(format!("read {}: {e}", path.display()))`, a 500 that leaks the host
  path) becomes a clean `AppError` with `StatusCode::NOT_FOUND` + code
  `FILE_NOT_FOUND` + an actionable, path-free message naming the filename and
  suggesting `list_files`.
- **ITEM-3**: the code_sandbox MCP `dispatch` (`code_sandbox/handlers.rs`) surfaces
  real client-class errors — map via `JsonRpcError::from_app_error(&app_err)`, but
  when the mapped code is `JsonRpcError::INTERNAL` fall back to the generic
  `"tool {name} failed"` (preserves the no-host-path-leak invariant for 5xx). The
  server-side `tracing::warn!` is kept.
- **ITEM-4**: `list_files` (`code_sandbox/tools/files.rs`) also surfaces
  model-authored artifacts — via a shared private helper
  (`conversation_model_authored_files(ctx)` = `model_authored_file_ids` +
  `get_by_ids_and_user`, reused by ITEM-1 so read and list can't drift) — appending
  `{name, size, is_file:true}` for each record whose filename is not already a
  workspace entry (workspace wins). The existing alphabetical sort orders the set.

## Files to touch

- `src-app/server/src/modules/code_sandbox/tools/files.rs` — ITEM-1, ITEM-2, ITEM-4
  (read fallback, clean 404, shared helper, list_files append); add `#[cfg(test)]`
  units where feasible.
- `src-app/server/src/modules/code_sandbox/handlers.rs` — ITEM-3 (guarded dispatch
  mapping).
- `src-app/server/tests/code_sandbox/tier3_http.rs` — error-surfacing + list_files
  tests.
- `src-app/server/tests/code_sandbox/` (tier2/tier3) — model-authored read/edit
  integration tests (new test fn(s) + a model-authored seed helper mirroring
  `tests/files_mcp/` seeding of `created_by='mcp'/'llm'` + `source_message_id`).

No changes to `get_conversation_files`, the bwrap bind-mount, or any public
signature.

## Patterns to follow

- **Model-authored resolution** — mirror `file/available_files.rs`
  (`model_authored_file_ids` ~L463 + `resolve_available_files` ~L501 which pairs it
  with `get_by_ids_and_user` at ~L576). Reuse the shared query; do NOT duplicate the
  provenance CTE (the exact drift PR #152 avoided).
- **Byte-load + ambiguity shape** — mirror the existing attachment branch in the
  same file (`files.rs:197-232`): `AMBIGUOUS_FILENAME` (400) on multi-match,
  `extension_of` + `get_file_storage().load_original` + `String::from_utf8` (→
  `BINARY_FILE` 400) on single match.
- **Error mapping** — mirror the sibling built-in MCP dispatch in
  `files_mcp/handlers.rs` + `memory_mcp/handlers.rs` (`JsonRpcError::from_app_error`,
  `code_sandbox/types.rs:134-147`).
- **Tests** — mirror `tests/code_sandbox/tier2_repository.rs` (DB seeding of
  user→conversation→branch→message→branch_messages→message_contents; the
  project-knowledge test at ~L366) and `tests/code_sandbox/tier3_http.rs` (the
  invalid_params assertion at ~L272); mirror `tests/files_mcp/` for seeding
  `created_by='mcp'/'llm'` files with `file_versions.source_message_id`.
