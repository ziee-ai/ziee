# DECISIONS — desktop-ui-guardrail-parity

All inputs resolved up front so implementation runs nonstop.

### DEC-1: For each backfilled guardrail, reuse `../../ui/scripts/` or copy into desktop?
**Resolution:** Reuse (package.json reference) when the script scans BOTH roots or
emits a SHARED artifact: `lint:adjacent-inline` → `../../ui/scripts/lint-adjacent-inline.mjs`
(dual-root, scans desktop/ui/src), `check:kit-manifest` → `../../ui/scripts/gen-kit-manifest.mjs`
(server-ui kit; desktop has none), `check:testid-registry` → `../../ui/scripts/gen-testid-registry.mjs`
(dual-root → shared registry), `check:design-spec` → `../../ui/scripts/gen-design-spec.mjs`
(shared repo-root DESIGN_SYSTEM.md). COPY into `desktop/ui/scripts/` when the
script is gallery-relative (`__dirname/../src/dev/gallery`): `gallery-geometry-audit.mjs`,
`affordance-audit.mjs`, `detector-acceptance.mjs`, `gen-crop-review-manifests.mjs`,
`gen-overlay-registry.mjs`.
**Basis:** codebase — matches desktop's existing split (references `../../ui/scripts`
for `lint:colors`; owns copies of `gen-state-matrix.mjs`, `gen-crawl-cassette.mjs`).

### DEC-2: `detector-acceptance.mjs` — port faithfully or adapt to desktop reality?
**Resolution:** ADAPT. The web harness renders a 488-line `DefectRepro.tsx`
`seeded-defect-repro` surface with 20+ CHAT-specific geometry repros (mermaid,
math, conversation-title, …) that have zero desktop meaning; desktop's gallery is
page-focused with no seeded surface. Porting that apparatus would be pure ceremony.
Instead the desktop harness proves the SHIPPED detectors are trustworthy two honest,
self-contained ways: (1) run the LINT detectors (`lint-icon-action.mjs` C11,
`lint-native-scroll.mjs` J8) against the copied `__detector_fixtures__/` and assert
each FIRES — needs no dev server; (2) a byte-identity DRIFT GUARD asserting the
desktop `gallery-geometry-audit.mjs` is byte-identical to the web source (whose own
`detector-acceptance.mjs` already validates the geometry detector against the full
repro set) — so the geometry detector is proven-correct by identity, and any future
divergence fails loudly. Still copy into `desktop/ui/scripts/`: `lint-icon-action.mjs`,
`lint-native-scroll.mjs`, `gallery-geometry-audit.mjs` (ITEM-2), and the
`__detector_fixtures__/` dir.
**Basis:** codebase + [[feedback_no_cosmetic_tests]] — verified `detector-acceptance.mjs`
requires `seeded-defect-repro`/`defect-repro-root` (server-only); adapting keeps the
meta-test meaningful without porting an irrelevant apparatus.

### DEC-3: How is the `lint:icon-action` CHECK gate wired (local copy vs reference)?
**Resolution:** `lint:icon-action` → run the LOCAL desktop copy
(`node scripts/lint-icon-action.mjs`) that DEC-2 already requires; its default
`ROOTS` resolve to `desktop/ui/src`, so it lints desktop-own source.
`lint:adjacent-inline` → REFERENCE `../../ui/scripts/lint-adjacent-inline.mjs`
(dual-root already covers desktop; no copy). `lint-native-scroll.mjs` is copied for
the detector meta-test ONLY (not added as a `check` gate — server-ui doesn't gate on
it in `check` either).
**Basis:** codebase — server-ui `check` omits `lint:native-scroll`; parity means
matching that omission.

### DEC-4: Where does `DEFECT_TAXONOMY.md` go for the crop script?
**Resolution:** `src-app/desktop/ui/docs/DEFECT_TAXONOMY.md` (the script reads
`path.resolve(UI_DIR,'docs/DEFECT_TAXONOMY.md')`, verified line 37), a verbatim copy
of `ui/docs/DEFECT_TAXONOMY.md`.
**Basis:** codebase — `gen-crop-review-manifests.mjs:37`.

### DEC-5: Which desktop overlays must be registered for `check:overlay-registry`?
**Resolution:** Only DOM overlay hosts under `desktop/ui/src/modules` are flagged by
the scanner. The single desktop-only DOM overlay host is
`layouts/app-layout/components/Drawer.tsx`; register it (or its trigger surface) in
the new `overlays.tsx` / `overlay-allowlist.json`. `file-dialog` uses the NATIVE
`@tauri-apps/plugin-dialog` (no DOM overlay) → not a registry surface. During
implementation, run `gen-overlay-registry.mjs --check` and register/allowlist EXACTLY
what it flags (no more, no less).
**Basis:** codebase — grep of desktop modules found one DOM Drawer host; file-dialog
store uses native `open()/save()`.

### DEC-6: Which surfaces get ITEM-7 chrome/overlay gallery coverage?
**Resolution:** Add desktop gallery story/overlay surfaces for the non-route
desktop-only chrome that the page grid can't reach: the `window` title-bar chrome,
the `layouts/app-layout` shell (incl. its Drawer), the `updater` update-available
dialog, and a `file-dialog` trigger surface. Render each via the mock-API cassette.
Route surfaces (settings-about, remote-access, host-mount, auth-magic) are ALREADY in
the page grid — do NOT duplicate them as stories.
**Basis:** codebase — verified route registration; chrome modules register `routes:[]`
or commented routes.

### DEC-7: Fix scope — desktop-only or shared source too?
**Resolution:** Fix ONLY desktop-own surfaces/source under
`src-app/desktop/ui/src/modules/{updater,tunnel-auth,remote-access,window,host-mount,
file-dialog,layouts,auth,desktop-base}` and desktop gallery/scripts. NEVER edit shared
`src-app/ui/src` (already audited + merged); a shared-source finding is out of scope
and recorded as "pre-existing / shared — not this feature".
**Basis:** user — task explicitly scopes to desktop-only surfaces.

### DEC-8: Vision crop pass — automated model or structured self-review?
**Resolution:** Generate the crop-review manifests (`gen:crop-review`) + capture the
desktop-only surface crops, then perform a STRUCTURED self vision-review answering the
`DEFECT_TAXONOMY.md` `[V]` (vision) ABSENCE questions per crop; record verdicts in
`DESKTOP_UI_FINDINGS.md`. No external vision model is wired into a headless gate on
this box (CLAUDE.md's UI Build Gate notes the vision pass is recorded out of band).
**Basis:** codebase/convention — CLAUDE.md "UI Build Gate" states criterion-1 vision
findings are recorded out of band; the machine-enforced criteria (2–4) are the gates.

### DEC-9: Do the new desktop gates BLOCK or WARN?
**Resolution:** BLOCK — chain them into `npm run check` so they fail the build,
identical to server-ui. Parity means enforcement, not advisory output.
**Basis:** user — task says "so they gate going forward (parity with server-ui's
check)".
