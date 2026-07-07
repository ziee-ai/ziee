# FIX_ROUND-2 — convergence re-audit

A fresh blind reviewer re-examined the FULL updated diff (all angles), with the
FIX_ROUND-1 changes in place.

## Verdict: security core CONFIRMED sound, 0 new confirmed defects

The reviewer independently verified:
- `sandbox="allow-scripts"` (no `allow-same-origin`) → null origin; SOP blocks
  parent/cookie/storage reach; TEST-4's `SCRIPT_EXECUTED` positive control proves
  it non-vacuously.
- The CSP prepend (`<!DOCTYPE html>${CSP_META}…`) genuinely removes both prior
  string-splice bypasses; every directive used is `<meta>`-honored; `connect-src`/
  `frame-src`/`object-src`/`worker-src` all fall back to `default-src 'none'`;
  `base-uri`/`form-action` close base/form. No new CSP bypass.
- No host-page HTML-injection surface (code view is React-escaped `{code}`;
  preview via React `srcDoc`; no `innerHTML`).
- Streaming guard + `referrerPolicy="no-referrer"` intact.
- TEST-3 (empty `seen` + `frame-ran` positive control) genuinely distinguishes
  "CSP blocked" from "frame never parsed."

No new CSP bypass, host XSS, sandbox escape, or state/a11y/error-handling defect
was found.

## Single new item — a documented accepted residual (not a code defect)

- **Preview busy-loop hang** (plausible, low): a `<script>while(1){}</script>`
  can hang the tab because a sandboxed iframe may share the main thread. The
  reviewer itself notes this is "inherent to any live-HTML-preview feature
  (jsfiddle/codepen have it)", "affects only the user who opted into Preview on
  their own chat", and causes "no data loss" — explicitly NOT ship-blocking. Its
  only actionable point was that it was undocumented. **Resolved by
  documentation:** added to the `htmlBlockSandbox.ts` accepted-residual list
  (alongside the self-navigation beacon), noting the DEFAULT Code view means it
  never triggers unless the user opts in per block. There is no proportionate
  code fix (a Worker/timeout sandbox is heavy and out of G3's scope).

No confirmed code defect remains; the two residuals (self-navigation beacon,
busy-loop hang) are both inherent to live-HTML preview and now fully documented.

**New confirmed findings:** 0
