# TEST_RESULTS — office-run-office-js

Backend-only diff (`src-app/desktop/tauri/**`; the pane JS is a bundled resource,
not a UI workspace; generated `openapi.json`/`types.ts` excluded), so the backend
integration chain applies — no `npm run check` gate line required.

## Automated (all green)

- **TEST-1**: PASS — `cargo test -p ziee-desktop --lib` `run_office_js_schema_requires_doc_and_script`.
- **TEST-2**: PASS — `tool_list_contains_exactly_the_seven_tools` (run_office_js in, edit_document out).
- **TEST-3**: PASS — integration `settings_mcp_test::test2_tools_list_returns_the_seven_office_tools`.
- **TEST-4**: PASS — `test4_edit_document_is_removed_unknown_tool`.
- **TEST-5**: PASS — `test5_run_office_js_invalid_args`.
- **TEST-6**: PASS — `test6_run_office_js_no_pane_is_not_connected` (422).
- **TEST-7**: PASS — integration `pane_rpc_test::run_office_js_dispatch_round_trip` (mock pane; text→content mapping asserted).
- **TEST-9**: PASS — node `taskpane.test.mjs` (serializeResult + describeError incl. hostile/throwing-getter cases).
- **TEST-10**: PASS — `test10_open_doc_full_name_desc_has_no_act_on_document` (OpenDoc schema).

Unit run: `cargo test -p ziee-desktop --lib office_bridge::` → 54 passed, 0 failed.
Integration run: `cargo test -p ziee-desktop --test integration_tests -- --test-threads=1
office_bridge::settings_mcp_test::test2_tools_list office_bridge::pane_rpc_test`
→ 9 passed, 0 failed, 3 ignored (the live tests below). No regression in the prior
pane-rpc suite.

## Live (macOS, `#[ignore]` — one manual Excel ribbon click each; run one-at-a-time)

- **TEST-8**: PASS — `run_office_js_live_mac_executes_script`. Ran live against a real
  Excel task pane: the hardcoded Office.js script set A1 and returned its address —
  `structuredContent.result == "Sheet1!A1"`. `1 passed; 0 failed` in 12.3s.
- **TEST-11**: PASS — `run_office_js_real_llm_live`. Real end-to-end: the coder.ziee
  `:4000` LiteLLM `qwen3.6-35b-a3b` model, given the shipped `run_office_js` schema,
  emitted a tool call whose Office.js `script` executed in the live pane (returned
  `"Sheet1!A1"`); the deterministic A1 read-back returned `"hello"`, proving the model's
  script actually set the cell. `1 passed; 0 failed`.

Run command (one at a time; quit the desktop app first so nothing holds 44300; SSH
tunnel up for TEST-11; between the two, close+reopen the task pane so it reloads the new
session token):

```bash
cd src-app
cargo test -p ziee-desktop --test integration_tests -- --ignored --nocapture \
  office_bridge::pane_rpc_test::run_office_js_live_mac_executes_script            # TEST-8
ZIEE_OFFICE_REAL_LLM_URL=http://127.0.0.1:4000/v1/chat/completions \
cargo test -p ziee-desktop --test integration_tests -- --ignored --nocapture \
  office_bridge::pane_rpc_test::run_office_js_real_llm_live                        # TEST-11
```
