# `__detector_fixtures__` — source-lint known positives

These `.tsx` files are **intentionally defective** source fixtures for the
Layer-3 source lints (taxonomy `[L]` classes). Unlike the runtime geometry
fixtures in `../DefectRepro.tsx`, these defects live in the SOURCE (a wrong icon
glyph, a raw native scrollbar) and are only detectable by an AST lint, not by
DOM geometry.

They exist so `scripts/detector-acceptance.mjs` can prove each source lint
actually FIRES on its known-bad instance. The lints EXCLUDE this directory from
their normal (repo-wide) scan — so `npm run check` stays green — and the
acceptance harness points each lint at this directory explicitly via
`--root=src/dev/gallery/__detector_fixtures__`, expecting ≥1 finding.

| File | Class | Miss | Defect |
|---|---|---|---|
| `IconActionMismatch.tsx` | C11 | #10b | "open in new tab" / "download" buttons render the wrong lucide glyph |
| `NativeScroll.tsx` | J8 | #17 | a raw `<div overflow-y-auto>` instead of the shared `<DivScrollY>` |

These are NEVER imported or rendered — they are lint fodder only.
