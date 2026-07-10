# PLAN — sandbox-tool-approval-loop

Fix: code_sandbox tool calls loop to "maximum iteration limit reached" when a local
gpt-oss/harmony model emits tool names WITHOUT the `<server_id>__` prefix (bare
`execute_command` or empty-prefix `__query_rag`) → empty server_id → NULL-server_id
approval row → `execute_approved_tools_sync` silently `continue`s without deleting →
re-found every iteration → max_iteration. Root cause empirically confirmed against the
live `ziee-web` container logs (the model's tool-call ids are actually UNIQUE
`chatcmpl-tool-*`; unique-id handling is a defensive net — see ITEM-2). See the approved
plan: `/home/khoi/.claude/plans/read-data-khoi-home-workspace-ziee-ziee-cheerful-karp.md`.

## Items

- **ITEM-1**: Fix C — in `execute_approved_tools_sync` (`mcp.rs:282-708`), make **every**
  non-executing branch both push an `is_error:true` tool_result for the tool_use_id AND
  delete the approval row (`Repos.chat.mcp.delete_tool_approval`) before `continue`: the
  `server_id == None` branch (currently a bare `continue` — the reported root of the loop),
  the server-not-found branch, the connect-fail branch, AND the sampling-no-session branch
  (all four are non-executing error branches in this function; the last three already pushed
  an error result but never deleted the row → same latent re-loop). Also push the tool_use_id
  into the returned `executed_tool_use_ids` in each (mirrors the success path). Guarantees the
  approved row clears on first pass and every tool_use ends with a matching tool_result, so
  the "Returning 0 approved tool results" spin cannot recur — a genuine failure surfaces a
  clear error instead of max_iteration.
- **ITEM-2**: Fix B — guarantee message-unique tool_use ids (defensive: the confirmed
  gpt-oss ids are actually UNIQUE `chatcmpl-tool-*`, but a model emitting an empty/duplicate
  id would collide on `UNIQUE(message_id,tool_use_id)` + the executed-id dedup). Add a pure
  helper `resolve_unique_tool_use_id(provider_id, used) -> String` that mints
  `format!("call_{}", Uuid::new_v4())` iff `provider_id` is empty OR already in `used`, else
  keeps `provider_id` (so good ids round-trip untouched). Apply it in `get_accumulated_content`,
  replacing `id: accumulated.id.unwrap_or_default()`. Seed `used` from the message's
  already-persisted tool_use ids (one `get_message_with_content(message_id)` near the top;
  degrade to empty set on DB error) to catch cross-iteration duplicates; iterate the drained
  accumulator entries sorted by `index` and insert each assigned id into `used` to catch
  within-stream duplicates. Also fix the misleading finalize log (`id={}` printed the
  content-type `"tool_use"` via `content_type()`, not the id — now prints the real id).
- **ITEM-3**: Fix A — recover server_id for prefix-less tool names. Add
  `tool_name_server_map: Arc<Mutex<HashMap<Uuid /*message_id*/, HashMap<String, Option<Uuid>>>>>`
  to `McpChatExtension` (`mcp.rs:259-277`), init in `new()`. Populate it in `before_llm_call`
  right after `all_tools` is built: split each advertised composed name on `"__"` into
  `(uuid, bare)`, inner map `bare -> Some(server_id)`, downgrade to `None` when a second
  distinct server advertises the same bare name. Add a pure helper
  `recover_server_id_for_bare_name(bare, &map) -> Option<Uuid>` (returns `Some` only for
  unambiguous hits). In `get_accumulated_content`, recover whenever the parsed prefix is **not
  a valid UUID** — this covers BOTH the no-`__` bare name (`execute_command`) AND the
  empty-prefix form (`__query_rag`), both of which gpt-oss/harmony actually emit. On hit set
  `server_id`; on ambiguous/not-found leave empty (→ ITEM-1 clear error). Clear the per-message
  map entry at the end of `get_accumulated_content` (symmetric with the accumulator drain).
- **ITEM-4**: Fix E — wire the new tests into a cheap `just` gate. Add a `check-mcp-approval`
  recipe (mirrors `check-sandbox-unit`, `justfile:107-110`) that runs
  `cargo test --lib mcp::chat_extension::` + `cargo test --test integration_tests --
  --test-threads=1 mcp_approval_loop_`, and append it to the `check:` dependency list
  (`justfile:73`). All new integration tests share the `mcp_approval_loop_` name prefix so
  one substring filter selects exactly them.

## Files to touch

- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (ITEM-1, ITEM-2, ITEM-3 + unit tests)
- `src-app/server/tests/mcp/mcp_approval_loop_test.rs` (NEW — ITEM-1/2/3 integration tests;
  `approval_test.rs` is unregistered/outdated, so a new registered file is used)
- `src-app/server/tests/mcp/mod.rs` (register the new test module)
- `justfile` (ITEM-4 gate target)

## Patterns to follow

- **ITEM-3 field/Mutex + lifecycle**: mirror the existing `tool_use_accumulator`
  (`mcp.rs:265` field, `mcp.rs:276` init in `new()`, lock scope + drain in
  `get_accumulated_content` `mcp.rs:2728-2745`). Same `Arc<Mutex<HashMap<..>>>` idiom,
  outer-key by `message_id`, never hold the lock across `.await`.
- **ITEM-1 error-result + delete**: mirror the existing error branch shape
  (`mcp.rs:354-370`, builds `McpContentData::ToolResult { is_error: Some(true), .. }`) and
  the existing delete call (`mcp.rs:674-678`, `delete_tool_approval(tool_use_id, message_id)`).
- **ITEM-2 pure helper**: place next to the other module-private helpers already unit-tested
  in `#[cfg(test)] mod tests` (`mcp.rs:2791`, e.g. `build_artifact_download_url`,
  `tool_system_guidance`). Finalization edit mirrors current `get_accumulated_content` shape.
- **ITEM-4 recipe**: mirror `check-sandbox-unit` (`justfile:107-110`) exactly (two `cargo
  test` lines, `cd src-app/server`).
- **Integration tests**: mirror `tests/mcp/approval_test.rs` and
  `tests/mcp/mcp_approval_workflow_test.rs` (TestServer + `MockMcpServer`/fixtures harness).
