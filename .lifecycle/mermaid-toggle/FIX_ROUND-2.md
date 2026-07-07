# FIX_ROUND-2 — convergence re-audit

An independent BLIND reviewer (fresh agent, diff-only, no design context) re-audited
the post-fix diff of `MermaidBlock.tsx` + the e2e spec across correctness /
error-handling / concurrency / state-management / a11y / tests-quality, focused on
the round-1 changes (per-render id, `isEmpty` branch, deferred revoke, ARIA roles,
download-bytes assertion).

## Result

The re-audit surfaced **no confirmed defect** — the round-1 fixes verified clean
(the unique-id ref mutation is inside the effect not render; the branch order is
correct; the dep array is sound; the ARIA roles are right; the e2e byte-read is
valid).

One **low / suspected** item was raised: the download anchor was not attached to
the DOM before `a.click()` (some engines historically ignore a programmatic click
on a detached anchor). This matches the app's existing shipping download idiom
(`chat/extensions/export/extension.tsx`) and is a non-issue on the desktop/e2e
Chromium target, but it is zero-downside to harden — so I applied
`appendChild(a) → click() → a.remove()`. No new logic or control flow was
introduced (a 2-line DOM attach/detach), so no further blind round is warranted.

## Verification

- `tsc --noEmit` (ui): exit 0.
- `lint:guardrails` (biome bans): pass.

**New confirmed findings:** 0
