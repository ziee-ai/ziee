# DECISIONS — resolved up front

### DEC-1: How wide is the fix — read tools only, read+list, or also the shell mount?
**Resolution:** read tools (`read_file`/`edit_file`) + `list_files`, reusing the
same `model_authored_file_ids` source; do NOT touch `get_conversation_files` or the
bwrap bind-mount (artifacts stay not `cat`-able in the `execute_command` shell).
**Basis:** user — khoi explicitly chose this scope after being shown the
narrow/read+list/wide options.

### DEC-2: Branch scoping of the model-authored resolution — active branch or whole conversation?
**Resolution:** whole conversation (all branches), via
`file::available_files::model_authored_file_ids(conversation_id, user_id)` verbatim.
**Basis:** convention — reuse the reviewed PR-#152 query rather than re-deriving;
`model_authored_file_ids` already spans all branches of the conversation, and
reusing it (vs. duplicating a narrowed CTE) is the anti-drift choice that PR #152
itself made.

### DEC-3: Error surfacing — swap dispatch to `from_app_error` wholesale, or guard it?
**Resolution:** map via `JsonRpcError::from_app_error(&app_err)` but keep the
generic `"tool {name} failed"` whenever the mapped code is `JsonRpcError::INTERNAL`;
change the terminal not-found in `load_file_content` from a 500 `io_err`
(host-path-leaking) to a clean 404 `FILE_NOT_FOUND` with a path-free actionable
message.
**Basis:** convention + security — mirrors `files_mcp`/`memory_mcp` dispatch, but the
`INTERNAL` guard preserves the existing deliberate no-host-path-leak invariant
(comment at `handlers.rs:268-271`); only client-class (4xx) messages, which the scan
confirmed carry no host path, are surfaced.

### DEC-4: How to produce `mcp` vs `llm` model-authored fixtures in tests?
**Resolution:** `mcp` fixtures via the real files-MCP `create_file` path (with an
`x-message-id` assistant turn → `created_by='mcp'` + `source_message_id` + staged
blob); the `llm` arm + the two-same-name ambiguity case via direct SQL/blob seeding.
**Basis:** codebase — mirrors `tests/files_mcp/mod.rs`
(`test_model_authored_file_is_readable_in_later_turn`) for the real path;
`created_by='llm'` has no first-class HTTP author path in code_sandbox tests, so it
is seeded (the resolver treats `mcp`/`llm` identically via `IN ('mcp','llm')`).

### DEC-5: New test file or extend `tier3_http.rs`?
**Resolution:** a new `tests/code_sandbox/tier3_read_artifacts.rs` (registered in
`tests/code_sandbox/mod.rs`), holding the artifact-read/edit/list/error tests + their
local author/seed helpers.
**Basis:** convention — the code_sandbox test dir is file-per-concern (tier3_http,
tier3_versions, tier3_concurrency, tier3_resource_limits); a focused file matches it
and keeps `tier3_http.rs` about auth/protocol.

### DEC-6: Does this introduce any operational tunable (limit / retention / toggle)?
**Resolution:** No. This is a bugfix that adds no resource limit, retention period,
quota, concurrency cap, feature toggle, or model/provider selection — so no
`*_settings` row, migration, REST, or admin card is introduced.
**Basis:** convention — the Phase-4 configurable-settings rule applies only when a
tunable is introduced; none is.

### DEC-7: `list_files` size field for a model-authored entry?
**Resolution:** use the file record's `file_size` (the `files.file_size` /
head-version size the `File` record already carries) for the `size` field; `is_file`
is `true`.
**Basis:** codebase — the `files` table has `file_size` (see `seed_file` in
`tier2_repository.rs`), so no extra fetch is needed; this matches the workspace
entries' `size` semantics closely enough for the model's purposes.

### DEC-8: On a workspace/artifact name collision in `list_files`, which wins?
**Resolution:** the workspace entry wins — a model-authored artifact whose filename
already appears among the workspace entries is skipped (no duplicate row).
**Basis:** convention — consistency with `read_file`, which is workspace-first; the
name the model would `read_file` is the workspace copy, so `list_files` must show
that one.
