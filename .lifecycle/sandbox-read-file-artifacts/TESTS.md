# TESTS — enumerated up front (bipartite ITEM ↔ TEST)

Backend-only diff (`src-app/server/**`) — no UI path, so no `tier: e2e` is
required by the gate; no new permission, so no `[negative-perm]` spec. All
integration tests drive the real HTTP path (`POST /api/code-sandbox`) against a
live `TestServer` (server subprocess with initialized `Repos` + file storage) —
the only place the model-authored resolution + storage load + JSON-RPC error
mapping run for real. `read_file`/`edit_file`/`list_files` need NO rootfs/bwrap
(only `execute_command` does), so these run without a mounted sandbox.

Model-authored fixtures are produced by a REAL author path (files-MCP `create_file`
with an `x-message-id` assistant turn → `created_by='mcp'` + `source_message_id`
provenance + staged storage blob), mirroring `tests/files_mcp/mod.rs`
(`test_model_authored_file_is_readable_in_later_turn`). The `llm` arm + the
ambiguity case use direct SQL/blob seeding.

## Tests

- **TEST-1** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: code_sandbox `read_file({filename})` returns the content of a model-authored **`mcp`** artifact (authored via files-MCP `create_file` in the same conversation); `structuredContent.text` contains the marker.
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: code_sandbox `read_file({filename})` returns the content of a model-authored **`llm`** artifact (seeded `created_by='llm'` + `source_message_id` + staged blob) — proves the `IN ('mcp','llm')` set, not just `mcp`.
- **TEST-3** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: `edit_file` on a model-authored artifact succeeds (copies into the workspace + applies the edit); a subsequent `read_file` reflects the edited content.
- **TEST-4** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: conversation-scope guard — a model-authored artifact created in conversation A is NOT readable via code_sandbox `read_file` in conversation B (error, and B's response never contains A's content marker). The data-leak guard on the widened set.
- **TEST-5** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: workspace-first — with a workspace file and a model-authored artifact sharing a name, `read_file` returns the WORKSPACE copy (write_file marker), proving the fallback runs only on a workspace miss.
- **TEST-6** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: `read_file` on a truly-missing filename returns JSON-RPC `invalid_params` (-32602) whose message names the filename + suggests `list_files` and contains NO host path (guards both the clean-404 conversion and the guarded dispatch mapping) — replaces the pre-fix opaque -32603.
- **TEST-7** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: `list_files` includes model-authored artifacts (both `mcp` and `llm`); a same-named workspace file produces exactly ONE entry (workspace wins, no duplicate).
- **TEST-8** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/code_sandbox/handlers.rs` — asserts: the pure error-mapping fn maps a 4xx `AppError` → `invalid_params` carrying the real message, and a 5xx `AppError` → the generic `"tool {name} failed"` (no inner message/host-path leaked).
- **TEST-9** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/code_sandbox/tier3_read_artifacts.rs` — asserts: two same-named model-authored artifacts (seeded) → `read_file` errors `AMBIGUOUS_FILENAME`, surfaced as JSON-RPC `invalid_params` (errors before byte-load; no blob needed).

## Coverage map

- ITEM-1 → TEST-1, TEST-2, TEST-3, TEST-4, TEST-5, TEST-9
- ITEM-2 → TEST-6
- ITEM-3 → TEST-6, TEST-8
- ITEM-4 → TEST-7

Every ITEM is covered; every TEST names a valid ITEM, tier, target file, and
assertion. Each test is **negative-controlled** at phase 8 (revert the fix →
confirm the test fails), per `ziee-negative-control-your-tests`.
