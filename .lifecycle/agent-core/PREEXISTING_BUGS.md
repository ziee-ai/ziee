# Pre-existing deterministic bugs surfaced by the agent-core audit

Scope: the OFF (legacy) baseline's deterministic (non-real-LLM) failures. These
are NOT agent-core regressions (they fail identically on clean `main`); they were
promoted to CONFIRMED findings and either fixed or surfaced under the expanded
"fix pre-existing bugs too" directive.

## Fixed (real, deterministic, root-caused — verified red→green OFF)

| # | Test | Root cause | Fix | Evidence |
|---|------|-----------|-----|----------|
| 1 | `path_resolution.rs` (whole binary) | `.embedded.expect()` moves a non-Copy field out of `&config` → build error → 0 tests ran | `.as_ref()` before `.expect()` | build-error → **6 passed** |
| 2 | `memory_mcp::test_recall_requires_memory_enabled` | Test assumed memory OFF-by-default, but schema **seeds `enabled=true`** (`202607140185_memory_schema.sql:17`, `202607145050_memory_seed.sql:4`). The recall guard (`handlers.rs:415`) is correct; the test simply never disabled memory, so recall returned `{memories:[]}` | Test now `UPDATE memory_admin_settings SET enabled=false` then asserts the `MEMORY_DISABLED` guard fires | FAILED → ok |
| 3 | `chat::test_single_assistant_message_architecture::test_single_assistant_message_with_tool_execution` | Helper parsed `GET /conversations/{id}/messages` as `Vec` — endpoint returns `PaginatedMessages` object | Extract `.messages` | `invalid type: map` → ok |
| 4 | `mcp::mcp_streaming_workflow_test::test_tool_results_in_api_history` | Same stale array-vs-object on the same endpoint (`body.as_array()`, stale "API returns array directly" comment) | Extract `.messages` | FAILED → ok |
| 5 | `mcp::elicitation_mcp_test::ask_user_accept_returns_the_answer_to_the_model` | `BUILTIN_SUFFIXES` allow-list missing `__run_js` (js_tool auto-attaches for tool-capable chats) | Added `__run_js` | FAILED → ok |

Product behavior in #2–#5 is correct; these were stale tests / a false test premise.

## Reported — needs a product scoping decision (NOT fixed here)

### files_mcp write tools never attach in an empty conversation (8 tests)

Affected (all `agentic_chat::*`, StubChat / deterministic):
`core_memory_block_is_injected_into_the_chat_request`,
`files_mcp_and_memory_coexist_in_one_conversation`,
`files_mcp_and_memory_combine_in_one_conversation`,
`model_authored_file_persists_and_is_reread_across_turns`,
`model_recalls_prior_result_via_get_tool_result`,
`multiple_builtin_subsystems_coexist_in_one_conversation`,
`multi_step_upload_analyze_mcp_edit_then_followup`,
`files_mcp_tool_call_is_recorded_as_built_in`.

**Root cause.** `attach_files_mcp` is set only when
`manifest_available = !files.is_empty()`:
- `src/modules/file/available_files.rs:292` — `META_FILES_MANIFEST_AVAILABLE = !files.is_empty()`
- `src/modules/file/chat_extension/file.rs:117` — `if tool_capable && manifest_available { … metadata.insert("attach_files_mcp","true") }`

So in a conversation with **no files yet**, the entire files_mcp tool set
(including the *write* tools `create_file`/`edit_file`/`rewrite_file`) is never
attached. The model therefore cannot author the first file — a chicken-and-egg:
you need a file to get the tool that creates a file. The memory+files
combination tests hit the same gate; `core_memory_block` is a distinct
core-memory-injection assertion in the same file.

Secondary: the StubChat harness (`tests/common/stub_chat.rs`) has **no
`create_file` plan arm** (only `sandbox_write_file*`), so `STUB_PLAN=create_file`
falls through to the default text-only response even if the tool were attached.

**Why not fixed here.** The corrective change — decouple files_mcp *tool
attachment* from *file presence* (attach when `tool_capable`, keep the manifest
*text* gated on file presence) — is a **core-chat product-behavior change**:
- It adds the files tool set to **every** tool-capable conversation, including
  empty ones, changing the OFF baseline broadly.
- It breaks other committed assertions that currently rely on files_mcp being
  ABSENT from an empty conversation (e.g. `elicitation_mcp_test`'s "only these
  built-ins attach with an empty mcp_config").
- Whether `create_file` *should* be offered before any file exists is a genuine
  product decision, not a contained bug.

This is outside the agent-core diff and would require a coordinated cross-test
re-baseline. **Recommendation:** greenlight it as a separate, scoped change
(attach files_mcp write tools when tool-capable regardless of file count, add a
`create_file` stub arm, and update the empty-config attach assertions), rather
than fold a wide-blast-radius core change into the agent-core cutover.

## Classified NOT bugs (model-flaky / environmental)

- `tool_call_history::chat_path_tool_call_records_source_chat` — fails at the
  `tool_starts > 0` GUARD the test documents as an LLM-miss detector; weak local
  Qwen didn't emit the tool call.
- `mcp_sampling::*`, `stdio_transport::test_stdio_list_server_tools`,
  `workflow_mcp::resources_test::resources_read_*`, `elicitation ..._real_llm_*`
  — differ between two runs of the SAME code (regress2 ↔ regress3), i.e.
  run-to-run flakes, not deterministic failures.
