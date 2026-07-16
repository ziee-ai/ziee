# DRIFT-1 — implementation vs plan (round 1)

Reconciling the shipped code against PLAN.md ITEM-1..4.

- **DRIFT-1.1** — verdict: none — ITEM-1 (read fallback) implemented in
  `load_file_content` exactly as planned: a shared `conversation_model_authored_files`
  helper (`model_authored_file_ids` + `get_by_ids_and_user`), filename filter, the
  `>1 → AMBIGUOUS_FILENAME` rule, single-match byte-load via
  `extension_of` + `get_file_storage().load_original(user, blob_version_id, ext)` +
  `String::from_utf8`. Runs only after workspace + `ctx.files` both miss
  (workspace-first + attachment-first preserved).
- **DRIFT-1.2** — verdict: resolved — the ambiguity message wording differs slightly
  from the attachment branch's (it points the model at the `files` MCP `read_file`
  by id rather than at `execute_command`/`cat`), because model-authored artifacts
  are NOT bind-mounted into the shell (DEC-1), so a `cat` suggestion would be wrong.
  This is the correct, scope-consistent guidance — an intentional refinement, not a
  deviation from intent.
- **DRIFT-1.3** — verdict: none — ITEM-2 (clean 404) implemented: the terminal
  not-found returns `StatusCode::NOT_FOUND` + `FILE_NOT_FOUND` + a path-free
  actionable message; the non-NotFound IO arm stays a 500 `io_err` (hidden by the
  guarded dispatch).
- **DRIFT-1.4** — verdict: none — ITEM-3 (guarded surfacing) implemented as the pure
  `map_tool_error(tool_name, &AppError)` fn (unit-testable, TEST-8) used by
  `dispatch`; `from_app_error` for client-class, generic `"tool {name} failed"` for
  `INTERNAL`. `tracing::warn!` kept.
- **DRIFT-1.5** — verdict: none — ITEM-4 (list_files) implemented: appends
  model-authored artifacts via the shared helper, deduped by name against workspace
  entries (workspace wins), size = `file_size` (DEC-7), before the existing sort.
- **DRIFT-1.6** — verdict: none — no change to `get_conversation_files`, the bwrap
  bind-mount, `openapi.json`, or any public signature, as planned. No migration.

**Unresolved drifts:** 0
