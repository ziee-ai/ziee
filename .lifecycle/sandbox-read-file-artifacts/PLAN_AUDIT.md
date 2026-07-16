# PLAN_AUDIT — plan vs codebase

## Breakage risk

- **ITEM-1 (read fallback)**: purely additive — a new branch that runs only when
  the workspace read AND the `ctx.files` match both miss (the current fall-through
  to the `Err` at `files.rs:234`). The existing workspace-first and
  attachment-first paths are unchanged, so no user-attachment or workspace read
  regresses. New dependencies (`model_authored_file_ids`,
  `Repos.file.get_by_ids_and_user`, `get_file_storage().load_original`,
  `extension_of`) are all already used by `resolve_available_files` /
  `load_file_content`'s attachment branch — no new public API. Risk: a
  model-authored file whose `blob_version_id` blob is missing on disk → the
  existing `load_original` error path (→ 500 `io_err`, hidden by ITEM-3). Acceptable
  (same failure mode as the attachment branch).
- **ITEM-2 (clean 404)**: changes only the status/code/message of the terminal
  not-found. No caller branches on the old `WORKSPACE_IO_ERROR`/500 for this case
  (the value flows into `dispatch` → `JsonRpcError`). `edit_file` also calls
  `load_file_content`; a missing file there now yields the same clean 404 (better,
  not worse). No breakage.
- **ITEM-3 (guarded dispatch)**: affects ALL code_sandbox tools' error envelopes,
  not just `read_file`. Mitigation: only client-class (4xx) errors get their real
  message surfaced; anything mapping to `INTERNAL` (all 5xx, incl. the host-path
  `io_err`s at `files.rs:234,236,272,276,334,354`) keeps the generic
  `"tool {name} failed"`. Scan of code_sandbox 4xx AppErrors found no host-path leak
  (only user-supplied path segments echoed back — safe). Behavior change is strictly
  more-informative for legit client errors; no security regression.
- **ITEM-4 (list_files)**: additive append + a dedup-by-name skip; workspace entries
  are unchanged and still win on name collision. `list_files` output grows to include
  artifacts — intended, and the JSON shape per entry (`{name,size,is_file}`) is
  unchanged, so no consumer contract breaks.

## Pattern conformance

- ITEM-1 mirrors `resolve_available_files` (`available_files.rs:501`) which pairs
  `model_authored_file_ids` with `get_by_ids_and_user` — the exact reviewed pairing.
  Ambiguity + byte-load mirror the sibling attachment branch in the same file
  (`files.rs:197-232`). Conformant.
- ITEM-3 mirrors `files_mcp/handlers.rs` + `memory_mcp/handlers.rs`
  (`JsonRpcError::from_app_error`). Conformant.
- ITEM-4 reuses the ITEM-1 helper (single source, no drift). Conformant.
- Tests mirror `tests/code_sandbox/tier2_repository.rs` + `tier3_http.rs` +
  `tests/files_mcp/` seeding. Conformant.

## Migration collisions

None. This change adds **no migration** (no schema change). Highest existing
migration `…158` is untouched; no collision surface.

## OpenAPI regen

Not required. No REST route, request/response type, enum, or permission is added or
changed — the edit is internal to the built-in code_sandbox MCP dispatch +
file-resolution logic. `openapi.json` / `api-client/types.ts` untouched; the diff is
backend-only and does not trip the phase-3/phase-8 frontend gates.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive fallback reusing `model_authored_file_ids` +
  `get_by_ids_and_user`; mirrors the attachment branch; no caller/signature change.
- **ITEM-2** — verdict: PASS — one-line error reshape (500+host-path → clean 404);
  no consumer depends on the old status.
- **ITEM-3** — verdict: CONCERN — broadest reach (all tools' error envelopes), but
  scoped to client-class messages with 5xx kept generic; host-path-leak scan clean.
  Resolved by the `INTERNAL`-guard; implement + verify with the tier3 no-host-path
  assertion.
- **ITEM-4** — verdict: PASS — additive, dedup-by-name, reuses the ITEM-1 helper;
  workspace entries unchanged.
