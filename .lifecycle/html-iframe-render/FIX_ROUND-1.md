# FIX_ROUND-1 — merge ledger, fix confirmed findings, re-audit

## Confirmed findings from the Phase-6 blind audit (LEDGER.jsonl) — all fixed

1. **CSP bypass: content before `<head>`** (security, medium) — a `<script>` before the user's `<head>` ran pre-CSP. **Fixed:** `buildSandboxedSrcdoc` now PREPENDS `<!DOCTYPE html>` + the CSP `<meta>` as the srcdoc's literal first bytes; nothing user-provided can precede it. (verify-fix.mjs POC: bypass closed.)
2. **CSP bypass: comment-decoy `<head>`** (security/correctness, medium) — a `<!-- <head> -->` comment trapped the meta inside a comment. **Fixed:** same prepend rewrite removed all regex/splicing into untrusted HTML.
3. **TEST-3 didn't prove the network block** (security/tests-quality, low) — asserted only the CSP string. **Fixed:** TEST-3 now attempts an external img + fetch to a sentinel host and asserts (via `page.on('request')`) that no request ever leaves the frame.
4. **TEST-4 positive control proved paint, not script-exec** (tests-quality, medium) — a dropped `allow-scripts` would still pass. **Fixed:** the inline script now rewrites `#hh` → `SCRIPT_EXECUTED`; TEST-4 asserts that value, so the isolation check can't pass vacuously.
5. **Code view lost highlighting + false "highlighted-ish" doc-comment** (perf/patterns-conformance, low) — **Fixed:** doc-comment corrected to state the plain `<pre><code>` choice + its rationale (mirrors the reference `syntax` CodeBlock; Streamdown's `CodeBlock` reads controls from context so embedding it would double the header chrome). Plain source is a documented, defensible decision (DEC-10).
6. **Uppercase / `htm` fences fell through** (correctness, low) — **Fixed:** renderer registered for `['html', 'htm']`; uppercase left as an accepted rare limitation (GFM fence info-strings are lowercase).

Rejected (with rationale, in LEDGER): iframe `bg-white` in dark theme (intentional neutral canvas for arbitrary HTML that assumes a white page); error-handling / state-management / concurrency / api-contract / i18n / perms-authz clean-sweeps.

## Re-audit (fresh blind reviewer on the full updated diff)

The re-audit CONFIRMED the fixes are sound (prepend guarantees CSP-first even for full-document HTML; `sandbox="allow-scripts"` intact; `['html','htm']` supported; `page.on('request')` sentinel is technically sound; TEST-4 SCRIPT_EXECUTED control real). It surfaced **2 NEW findings**, both now fixed:

- **RE-1 — self-navigation beacon** (security, medium): the CSP blocks external resource loads but cannot stop the frame navigating ITSELF (`location=…` / `<meta refresh>`) — no platform primitive covers same-frame navigation. The prior doc/CSP claim "blocks ALL external network / no exfiltration" was therefore an overclaim. **Fixed by precise documentation:** the module doc + `CSP` comment + DEC-5 now state the residual exactly (a null-origin beacon can carry only attacker-authored data; nothing sensitive exists to leak — isolation, not egress-block, is the guarantee). No platform fix exists; this is the correct resolution.
- **RE-2 — TEST-3 could pass vacuously** (tests-quality, low): `seen===[]` had no positive control that the img/fetch was actually attempted. **Fixed:** TEST-3 now includes an inline script that sets `#ran`→`frame-ran` and asserts it, proving the frame loaded + executed before asserting the sentinel host is unseen.

**New confirmed findings:** 2
