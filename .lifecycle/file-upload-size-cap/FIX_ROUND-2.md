# FIX_ROUND-2

## Fix applied (from FIX_ROUND-1's confirmed finding)
- **tests-quality (medium) — 413 assertion flakiness.** Made the over-body-limit
  assertion race-tolerant: `match` on the reqwest result — `Ok(resp)` asserts
  status == 413; `Err(e)` asserts `!e.is_timeout()`. A clean 413 OR a mid-stream
  connection reset both prove the body-limit layer engaged; a handler response
  (201 / 400 FILE_TOO_LARGE) still fails the test. (`tests/file/mod.rs`)

## Re-audit (blind, on the fix diff `HEAD~1...HEAD`)
A fresh blind reviewer confirmed the primary guard is intact — a hardcoded 200 MB
body limit (the regression this test exists to catch) returns `Ok(400)` and fails
the `assert_eq!(413)`; all HTTP status errors (500/400/201) arrive as `Ok(resp)`
and hit the strict 413 assertion.

- **(low, dismissed) Err branch over-permissive.** A server panic/OOM that resets
  the connection mid-upload would also surface as a non-timeout transport `Err` and
  pass. Dismissed: reqwest exposes no API to distinguish a body-limit reset from a
  crash reset, so this cannot be tightened without either reintroducing the
  flakiness this fix removed or writing a platform-brittle error-kind assertion; a
  crashed server would independently fail many other tests. Accepted trade-off, not
  actioned.

**New confirmed findings:** 0
