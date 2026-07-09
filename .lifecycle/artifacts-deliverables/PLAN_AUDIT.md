# PLAN_AUDIT — artifacts-deliverables (v3: WYSIWYG canvas)

Audit of the v3 plan against the codebase. Backend (ITEM-1..5) is unchanged from v2
and low-risk; the WYSIWYG editor (ITEM-6..8, 11) is the new risk surface — a first-of-
its-kind dependency in an app that currently has NO editor primitive.

## Breakage risk

- **Backend additive.** ITEM-1/3/4/5 are new routes; ITEM-2 a new fn. No signature
  changes. ITEM-1 reuses `commit_new_version` (already the append point + row-locked in
  `append_version`), so user + model writes serialize safely; all versions kept.
- **New editor dependency (ITEM-6) is the biggest risk.** `platejs` pulls Slate +
  remark + a component tree. Risks: (a) **bundle size** — mitigated by lazy-loading
  (mirror `LazyStreamdown`, so the editor never loads until Edit is entered); (b)
  **biome guardrails** — Plate components use refs/DOM; they must be adopted into the
  kit so the raw-DOM/antd bans don't trip (adoption, not raw usage); (c) **syncpack
  drift** — the dep must be added at identical versions to `ui` and `desktop/ui`
  (`.syncpackrc.json` lint), an explicit ITEM-12 check; (d) **React/TS peer versions**
  — Plate must match the repo's pinned React/TS `overrides` (verify at add time).
- **Markdown round-trip fidelity (ITEM-7)** is a correctness risk: the app **renders**
  with Streamdown but would **edit** with Plate — two markdown engines. A construct that
  round-trips lossily (a GFM table edge case, a footnote, raw HTML/MDX) could silently
  alter the file on save. Mitigations: constrain the editor to the Streamdown-rendered
  GFM subset; **preserve unknown constructs verbatim** (never drop); normalize-on-save
  for stable diffs; a dedicated round-trip fidelity test (TEST-3) + a parity check that
  editor output re-renders identically under Streamdown for the supported subset.
- **Shared `file` panel edit (ITEM-8)**: the view/edit toggle appears on every file;
  gated so only `markdown` gets the WYSIWYG (code/csv/binary stay view+export). Must
  render an arbitrary `fileId` editable outside the file drawer.
- **a11y (ITEM-11)**: a formatting toolbar adds many icon buttons — each needs an
  accessible name or the `gate:ui` a11y-name check (MEDIUM) and axe fail. Budgeted.
- **Frontend regressions**: the `file` panel type/data is unchanged (`{fileId,version?}`),
  so persisted tabs and existing openers keep working; `rehydrateTabs` unaffected.

## Pattern conformance

- **ITEM-1/2/3/4/5** conform (see v2 — restore_version / pandoc / content_disposition /
  summarizer / available_files mirrors).
- **ITEM-6** conforms to the repo's shadcn component-ownership model (adopt Plate's
  shadcn components into `components/kit/` rather than consuming a black-box widget) and
  the `LazyStreamdown` lazy-load idiom. Reuse-first is honored by running
  `shadcn-component-discovery` before authoring (ITEM-11).
- **ITEM-7** is new territory (no existing markdown-serialization code beyond Streamdown
  rendering), so it defines its own utility with explicit fidelity tests — acceptable.
- **ITEM-8** mirrors `CoreMemoryBlocksEditor` edit→save→REST + the `file` panel pointer
  pattern.
- **ITEM-9** mirrors the literature `tool_result`→`displayInRightPanel` pattern.
- **ITEM-11/12** follow the design-system skills + both-workspace regen conventions.

## Migration collisions

- **None — no migration.** Versioning, permissions (file + `conversations::read`), and
  the conversation↔file association (`source_message_id`) all exist. No new table, no
  new column, no `created_by` vocabulary change. `ls migrations/` tail `131` untouched.

## OpenAPI regen

- **Required (endpoints only).** The four new endpoints regen into `Api.*` + `types.ts`
  in BOTH workspaces; `deliverables` reuses the existing `File` schema (no new domain
  type). `SyncEntity` unchanged (reuses `File`). The editor + round-trip are pure
  frontend (no API surface). `emit_ts` golden-parity gates the regen.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `restore_version` mirror; the missing user-write primitive; row-lock serializes writers.
- **ITEM-2** — verdict: PASS — `convert_to_docx` copies `convert_to_pdf`; docx native writer (smoke-tested).
- **ITEM-3** — verdict: PASS — user download in a chosen format; reuses pandoc + `content_disposition`.
- **ITEM-4** — verdict: CONCERN — new conversation→markdown serializer must faithfully handle every `MessageContentData` variant; bounded, per-variant unit test.
- **ITEM-5** — verdict: CONCERN — deriving deliverables must reuse the `available_files` ownership join or risk a cross-user leak; ownership integration test.
- **ITEM-6** — verdict: CONCERN — a new heavyweight editor dep: bundle (lazy-load), biome (kit adoption), syncpack (both workspaces same version), React/TS peer pins. All are mitigable + explicitly budgeted; none blocks.
- **ITEM-7** — verdict: CONCERN — markdown round-trip fidelity across two engines (Plate edit vs Streamdown render) is a real correctness risk; mitigated by a constrained subset, verbatim-preserve of unknowns, normalize-on-save, and a fidelity + render-parity test.
- **ITEM-8** — verdict: CONCERN — edits the shared `file` panel; must gate WYSIWYG to `markdown` and render an arbitrary editable `fileId`; covered by unit predicate + e2e.
- **ITEM-9** — verdict: PASS — literature `tool_result`→`displayInRightPanel` mirror; first-appearance-only auto-open.
- **ITEM-10** — verdict: PASS — small menus in existing header slots.
- **ITEM-11** — verdict: CONCERN — design-system + a11y + gallery/state-matrix/testid/kit gates for a large new component surface; the toolbar's per-control accessible names are the main a11y load; budgeted as its own item + a gallery e2e.
- **ITEM-12** — verdict: CONCERN — regen + desktop mirror of the editor + `npm run check` both workspaces (incl. syncpack) are hard gates; endpoints-only API keeps the type surface small.
