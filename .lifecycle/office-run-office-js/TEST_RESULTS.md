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

## Live (macOS, `#[ignore]` — require a manual Excel ribbon click; NOT auto-runnable)

- **TEST-8**: PENDING — `run_office_js_live_mac_executes_script`. Infrastructure verified
  ready (bridge listened on 44300, Excel opened, `ziee-office-bridge.manifest.xml`
  sideloaded); blocked only on the human clicking `Home → Ziee → Show Ziee Bridge` to
  open the add-in task pane (an Office add-in pane cannot be opened by automation).
- **TEST-11**: PENDING — `run_office_js_real_llm_live`. Real-LLM half INDEPENDENTLY
  verified: the coder.ziee `:4000` LiteLLM `qwen3.6-35b-a3b` model, given the shipped
  `run_office_js` schema, returns a well-formed tool call with valid Office.js (checked
  live via curl). SSH tunnel (`4000:127.0.0.1:4000`) up + reachable. Blocked only on the
  same one manual ribbon click for the live-execution half.

**To finish TEST-8 / TEST-11** (one at a time; quit the desktop app first so nothing
holds 44300; the SSH tunnel must be up for TEST-11):

```bash
cd src-app
# TEST-8 (no LLM):
cargo test -p ziee-desktop --test integration_tests -- --ignored --nocapture \
  office_bridge::pane_rpc_test::run_office_js_live_mac_executes_script
# TEST-11 (real LLM):
ZIEE_OFFICE_REAL_LLM_URL=http://127.0.0.1:4000/v1/chat/completions \
  cargo test -p ziee-desktop --test integration_tests -- --ignored --nocapture \
  office_bridge::pane_rpc_test::run_office_js_real_llm_live
```
Each opens Excel and waits up to 600s; click the ribbon button when it prompts.
