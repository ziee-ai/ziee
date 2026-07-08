# TEST_RESULTS — lazy-load-conversation-messages

All phase-3 enumerated tests authored + run. Backend integration + UI unit + all
e2e green; static UI gate green.

## Frontend static gate

- npm run check (ui): PASS

(`src-app/desktop/ui` was not touched except the mechanically-generated
`openapi.json`/`api-client/types.ts`, which the lifecycle excludes from UI-work
determination — the chat module does not exist in the desktop workspace.)

## Unit

- **TEST-1**: PASS — `cargo test -p ziee --lib chat::core::types::message::tests` (limit clamp + cursor mutual-exclusion)
- **TEST-2**: PASS — `node --test scrollAnchor.utils.test.ts` (pickTopAnchor + restoreDelta + boundary)
- **TEST-3**: PASS — `node --test messageWindow.test.ts` (prepend/append/mergeTail order + dedup + empty-page)
- **TEST-13**: PASS — `openapi::emit_ts::tests::types_ts_parity` (regen parity; ran in the lib suite)
- **TEST-14**: PASS — `cargo test -p ziee --lib chat::core::types::message::tests` (search query clamps + blank + snippet bounds + Unicode panic-guard)

## Integration

- **TEST-4**: PASS — `chat::messages_test::test_history_pagination_tail_and_before`
- **TEST-5**: PASS — `chat::messages_test::test_history_pagination_around_and_after`
- **TEST-6**: PASS — `chat::messages_test::test_history_pagination_validation_and_errors`
- **TEST-7**: PASS — `chat::branches_test::test_pagination_follows_active_branch`
- **TEST-8**: PASS — `chat::messages_test::test_history_window_content_association`
- **TEST-15**: PASS — `chat::messages_test::test_in_conversation_search`

(Run: `cargo test --test integration_tests -- --test-threads=4 <names>` → 6 passed;
log at `/data/pbya/ziee/tmp/lifecycle-logs/lazyload-int.log`.)

## E2E

- **TEST-9**: PASS — `tests/e2e/chat/lazy-load-messages.spec.ts` (recent-first load; older prepend; scroll-anchor invariant: scrollTop grows by the prepended height)
- **TEST-10**: PASS — `tests/e2e/chat/lazy-load-jump-to-message.spec.ts` (`#message-<id>` deep-link centers + highlights an unloaded message; scroll-down pages newer)
- **TEST-11**: PASS — `tests/e2e/chat/lazy-load-branch-reset.spec.ts` (branch switch resets the window to the new branch tail)
- **TEST-12**: PASS — `tests/e2e/chat/lazy-load-sse-and-short.spec.ts` (both cases: a new turn appends at the bottom keeping older pages; a short conversation renders fully + fires no `before=` request)
- **TEST-16**: PASS — `tests/e2e/chat/conversation-find.spec.ts` (server-side find surfaces a match in an unloaded message + jumps to it via around=)

(Run: `npx playwright test <5 specs> --workers=1` → 6 passed [TEST-12 is 2 cases].)
