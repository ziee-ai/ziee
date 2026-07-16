# FIX_ROUND-2 — fresh blind round on the FIX_ROUND-1 fixes

A new blind auditor (diff-only context, no knowledge of round 1 and none of my reasoning)
re-reviewed the whole diff across security / concurrency / correctness / error-handling /
regressions / tests-quality.

## Independent verification of the round-1 HIGH fix — CLEAN

The auditor re-derived the locking from first principles rather than trusting my comment, and
confirmed it — including a subtlety I had not spelled out: `UPDATE users SET token_version = …`
touches no key column, so it takes **FOR NO KEY UPDATE**, which **does** conflict with the claim's
**FOR SHARE** (*and would NOT if the code had used `FOR KEY SHARE`*). Both interleavings verified
safe, the rotation-grace path covered, and **no deadlock**: both transactions order
`users → refresh_tokens`; two concurrent refreshes take compatible FOR SHARE locks; the successor
INSERT's FK check wants FOR KEY SHARE on the already-held stronger lock; and the only other writers
of both tables (change-password, admin reset) issue autocommit statements, not multi-statement
transactions, so no cycle exists. Extractor coverage and the SSE epoch re-checks also came back
clean.

## Fixed this round

- **MEDIUM — TEST-23 was a probabilistic guard sold as a proof.** The auditor showed logout reaches
  its `UPDATE users` in ~3 round-trips while refresh needs ~5 to reach its successor INSERT, so the
  natural ordering usually has logout win and the window is never exercised — meaning the test would
  likely go GREEN with the lock removed. Added **TEST-25**, which is deterministic: hold the exact
  lock a logout holds, prove a real `/auth/refresh` BLOCKS on it, release it, prove it then
  succeeds. **Negative-controlled — with the `FOR SHARE` removed it FAILS** with the intended
  diagnostic. TEST-23 is kept (it can only ever fail for a real reason) with its probabilistic scope
  documented in the spec.
- **LOW — `permissions: []` in `setAuthFromAutoLogin` was dead for its stated purpose and actively
  harmful.** VERIFIED against the callers myself: the only identity-changing caller
  (`AuthCallbackPage.tsx:106`) passes `user: null` and takes the early-return, so it never reaches
  that `set()` — my "authenticated render window with foreign permissions" rationale was simply
  wrong. The callers that DO reach it (desktop `applyTokens`, tunnel `applySession`) re-mint the
  SAME identity and never call `initAuth()`, so I had pinned `permissions: []` for their entire
  session — masked only by the `is_admin` short-circuit, and a real breakage for any non-admin
  desktop/tunnel identity. Reverted; TEST-18 rewritten to pin the corrected behavior, TEST-18b added
  to pin why it is safe.
- **INFO — `end_session_atomically`'s comment overclaimed.** It said a racing refresh is serialized
  by its `refresh_tokens` row lock; that lock is precisely what is INSUFFICIENT (it is the whole
  reason the HIGH existed). Corrected to name the `users` lock, and to state the guarantee's real
  scope: it covers ROTATION, not `mint_session_tokens`/`register` — a LOGIN racing a logout may
  leave its fresh token active, which is deliberate and benign (a login is a fresh authentication,
  not a session this logout was meant to end).
- **INFO — the logout doc undercounted its residues.** Download tokens (`DownloadTokenClaims`,
  `aud: ziee-download`, 1h TTL) are a second credential with no epoch. Documented as RESIDUE 2.
- **LOW — a misleading e2e title.** "tears down the other tab without a reload" — the app's own
  teardown *does* reload; only the TEST drives no reload. Retitled "on its own".
- **LOW — TEST-19 is non-discriminating** (the pre-existing wipe already nulled `token`, so it passes
  on base unchanged). Kept as a regression guard for the teardown refactor, with its honest scope
  stated in the spec so no one mistakes it for evidence of the revocation.

## Accepted / noted, not fixed

- **The chat-stream epoch teardown has no test** (the sync one does, via the `SYNC_RECHECK_TICK_MS`
  seam + TEST-24). Its re-check is line-for-line symmetric with the tested sync path; covering it
  would mean adding a debug seam to production code purely for testability. Noted in STATUS rather
  than silently ignored.
- **`Auth.store.test.ts` runs in no automatic gate** — this workspace has no `vitest` npm script and
  `npm run check` / `gate:ui` do not invoke it (7 other `*.store.test.ts` files are in exactly the
  same position; `vitest.config.ts` exists precisely for them). A base gap, not this feature's to
  close (B3/B6: don't add a gate that reads from a stripped path, don't edit shared config to suit
  one feature). The specs were run and negative-controlled by hand; recorded in TEST_RESULTS.
- **`revoke_all_for_user` has the same unlocked-revoke race** for change-password / admin reset, and
  neither bumps the epoch. Pre-existing and explicitly descoped in DEC-8 — recorded in STATUS as its
  own ticket, with the note that a password change is arguably where it matters most.

**New confirmed findings:** 0
