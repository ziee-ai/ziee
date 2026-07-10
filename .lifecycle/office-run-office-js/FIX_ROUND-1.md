# FIX_ROUND-1 — office-run-office-js

## Fixes applied for the Phase-6 ledger (9 confirmed findings)

- **describeError TypeError** (taskpane.js) — coerce `msg`/`name`/`code` via a
  non-throwing `safeString`, so a thrown value with a non-string `.message`+`.name`
  no longer makes `msg.indexOf` throw inside the `.catch`.
- **serializeResult "never throws" + redundant re-parse** (taskpane.js) — degrade
  non-serializable values via `safeString` (never throws); on the non-truncated path
  return the native `value` (reply() re-serializes it identically) instead of a
  redundant `JSON.parse`.
- **Header comment paren + redundant "via Office.js" + ITEM-9 tag** (taskpane.js) —
  rewrote the responsibilities comment; dropped the lifecycle tag.
- **Stale "three" comment** (handlers.rs) — the 2-pattern arm comment no longer says
  "three"; documented run_office_js routing (same `broker::call_pane` + pane-side
  `sameDoc` guard as the other pane tools) and the runaway-sync-script limit (bounded
  for the caller by `CALL_TIMEOUT`).
- **Tests strengthened** — describeError node test now asserts the name is surfaced,
  the name-in-message dedup branch, and the hostile non-string message; the mock-pane
  run_office_js test asserts the `text`→content mapping; the real-LLM test adds a
  deterministic A1 read-back; a one-at-a-time doc note added to the live-test helper.

Rejected findings (false positives / inherent / pre-existing) are recorded with
rationale in `LEDGER.jsonl` (10 rejected): the `StatusCode`/`DISPATCH_METHOD`
"orphaned import" claims (both still used — verified by grep), the sole-pane
write-routing (pre-existing model shared with add_comment, mitigated by the pane
guard), the sync-loop wedge (inherent to arbitrary code in a single-threaded
WebView), the fixed-port live-test collision (inherent — manual one-at-a-time).

## Re-audit round (2 blind reviewers on the fixed diff)

Two NEW confirmed findings surfaced and were fixed this round:

- **describeError throwing-getter** (taskpane.js) — bare property reads
  (`e.message`/`e.name`/…) on a fully model-controlled thrown value can trip a
  hostile throwing getter (`throw { get message(){ throw 0 } }`), throwing inside the
  `.catch` and swallowing the reply. FIX: wrapped the whole `describeError` body in a
  try/catch with a generic fallback; added a throwing-getter regression test.
- **real-LLM A1 verify too weak** (pane_rpc_test.rs) — the read-back only asserted
  A1 non-empty, but the helper reuses an existing workbook without clearing A1, so a
  stale value from another live test could mask a failed model write. FIX: assert A1
  contains the requested `hello` (case-insensitive).

Both fixes verified: node `taskpane.test.mjs` green (incl. the new throwing-getter
case), desktop unit tests green, integration tests compile.

**New confirmed findings:** 2
