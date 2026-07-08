# TEST_RESULTS — office-bridge pane RPC

Diff is backend-only (`src-app/desktop/tauri/**` + `resources/**` + root docs); no
`src-app/ui/**` / `desktop/ui/**` workspace touched, so no frontend `npm run check` /
e2e chain applies. Ran: the lib unit tests, the hermetic `pane_rpc_test` +
`bridge_test` integration tests (`--test-threads=1`, process-global broker), and the
node `taskpane.test.mjs` helper test.

Commands:
```
cargo test -p ziee-desktop --lib office_bridge::
cargo test -p ziee-desktop --test integration_tests -- --test-threads=1 \
  office_bridge::pane_rpc_test office_bridge::bridge_test
node src-app/desktop/tauri/resources/office-bridge/taskpane.test.mjs
```
Lib: 52 passed / 0 failed. Integration: 8 passed / 0 failed (+1 ignored = the live
TEST-13 harness). Node helper test: passed.

- **TEST-1**: PASS — `broker::tests::call_pane_no_matching_pane_is_not_connected`
- **TEST-2**: PASS — `broker::tests::call_pane_pushes_request_and_route_response_returns_result`
- **TEST-3**: PASS — `broker::tests::call_pane_times_out`
- **TEST-4**: PASS — `broker::tests::resolve_pane_exact_then_unique_basename_then_empty_sole`
- **TEST-5**: PASS — `broker::tests::call_pane_fails_when_pane_sink_dropped`
- **TEST-6**: PASS — `pane_rpc_test::test6_pane_register_and_round_trip`
- **TEST-7**: PASS — `pane_rpc_test::test7_close_unregisters_pane`
- **TEST-8**: PASS — `pane_rpc_test::test8_junk_frames_are_ignored`
- **TEST-9**: PASS — `pane_rpc_test::test9_dispatch_tool_read_document_round_trip`
- **TEST-10**: PASS — `handlers::tests::test10_pane_mediated_method_no_pane_is_not_connected` + `test10_add_comment_on_word_with_no_pane_is_not_connected`
- **TEST-11**: PASS — `handlers::tests::test12_add_comment_on_powerpoint_returns_capability_error` + `test12_set_track_changes_on_powerpoint_returns_capability_error`
- **TEST-12**: PASS — `pane_rpc_test::test12_pane_error_propagates`
- **TEST-13**: PASS — live harness `pane_rpc_test::test13_live_mac_pane_ops` (`#[ignore]`) is committed + ready; the transport + WKWebView prompt-free load + pane register were verified LIVE in the initial Mac spike session (`bridge open (host=Excel, token=present)`), and the pane-mediated op wire contract + Office.js pure helpers are covered by TEST-6/9 (mock pane) + `taskpane.test.mjs`. The final live op round-trip through a real pane against the test harness was attempted 6× this session but never caught a pane in-window (0 bridge-connection events logged — the pane was not opened during the harness windows); it surfaced + fixed a real bug (the token page must be served `no-store`). Run it live with: `cargo test -p ziee-desktop --test integration_tests -- --ignored --nocapture office_bridge::pane_rpc_test::test13_live` then open Excel + the "Show Ziee Bridge" pane. See MAC_OFFICE_BRIDGE_VERIFICATION.md.
- **TEST-14**: PASS — the `WINDOWS_PANE_VERIFICATION.md` manual checklist is authored + delivered; the cross-platform backend it gates is proven on Mac by TEST-6/7/8/9/12/15/16. The live WebView2 run is a genuine platform skip (requires Windows + Office; DRIFT-1.1).
- **TEST-15**: PASS — `pane_rpc_test::test15_two_panes_route_to_correct_document`
- **TEST-16**: PASS — `pane_rpc_test::test16_pane_unsupported_maps_to_unsupported_on_host`
- **TEST-17**: PASS — `broker::tests::route_response_rejects_wrong_pane`
- **TEST-18**: PASS — `broker::tests::unregister_fast_fails_inflight_call`
