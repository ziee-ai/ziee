# PLAN_AUDIT — artifacts-deliverables

Audit of PLAN.md against the codebase. The plan is reuse-heavy: the backend is almost
entirely additive on top of existing file/version/pandoc/sync primitives; the risk
concentrates in the three new frontend editors and the one new migration.

## Breakage risk

- **Backend is additive.** New routes (append-version, file export, conversation export,
  deliverables list + pin/unpin), one generalized converter fn (`convert_to`), one new
  `SyncEntity` variant, one new migration, one new serializer module. No existing
  signature changes.
- **`commit_new_version` is the shared append point** (ITEM-1). User writes are
  indistinguishable downstream from the `files_mcp` edit tools' writes; `append_version`
  already `SELECT … FOR UPDATE` row-locks, so a user save racing a model `edit_file`
  serializes — last writer becomes a new head, nothing lost (all versions restorable).
- **New migration `132`** (ITEM-17) is a pure link table (`ON DELETE CASCADE` both FKs);
  deleting a conversation or file cascades the curation row, never orphans, mirroring
  `project_files`. No ALTER of an existing table.
- **`SyncEntity::Deliverable`** (ITEM-17): adding a variant forces no audience at compile
  time — the emit sites must use `Audience::owner(conversation_owner)` (never broader),
  matching the `conversations::read`+ownership gating on the refetch endpoint.
- **Two new editor dependencies** (Plate ITEM-6, CodeMirror ITEM-19) are the largest
  risk: bundle (both lazy-loaded, so never loaded for view-only users), biome guardrails
  (adopted into the kit, not raw), syncpack cross-workspace version parity, and
  React/TS peer pins (verified at add time). All mitigable; none blocks.
- **Markdown round-trip across two engines** (ITEM-7): the app renders with Streamdown
  but edits with Plate; a lossy round-trip could silently alter a file on save.
  Mitigated by constraining to the Streamdown-rendered GFM subset, preserving unknown
  constructs verbatim, normalize-on-save, and a fidelity + render-parity test. Code
  (plain text) and CSV (reuse the existing parser) carry lower/handled round-trip risk.
- **Shared `file` panel edit** (ITEM-8): the view/edit toggle appears on every file;
  edit is gated to the type-appropriate editor (markdown/code/csv), others stay
  view+export; must render an arbitrary editable `fileId` outside the file drawer.
- **Selection→edit** (ITEM-16) must not mis-edit: a non-unique selection degrades to
  instruction-only rather than an ambiguous `old_str`; it only shapes the request — the
  edit still flows through `files_mcp::edit_file` (append-only, restorable, no trust
  boundary change).
- **a11y** (ITEM-11): editor toolbars + selection popovers add many controls, each
  needing an accessible name or `gate:ui` a11y/axe fails — budgeted as its own item.
- **Frontend regressions**: the `file` panel type/data is unchanged (`{fileId,version?}`);
  persisted tabs + existing openers keep working; `rehydrateTabs` unaffected.

## Pattern conformance

- ITEM-1 mirrors `versions::restore_version`. ITEM-2/3/4/23 reuse `find_pandoc`/
  `convert_to_pdf` + `content_disposition` + `workspace_export`. ITEM-4 extends
  `summarizer::message_to_summarizable`. ITEM-5 mirrors `available_files` scoping.
  ITEM-17 mirrors `project_files` + `SyncEntity::File` emit.
- ITEM-6/7/8/19/20/21 follow the shadcn component-ownership model the kit already uses,
  the `LazyStreamdown` lazy-load idiom, `CoreMemoryBlocksEditor`'s edit→save→REST, the
  `file` panel pointer pattern, and (for CSV) the existing tabular-viewer parser.
- ITEM-9 mirrors literature's `tool_result`→`displayInRightPanel`. ITEM-13/14/18/22
  reuse the tabbed panel + `sync:<entity>` self-gated refetch + the per-version text
  endpoint. ITEM-11/12 follow the design-system skills + both-workspace regen convention.
- Reuse-first is honored: `shadcn-component-discovery` runs before authoring new UI
  (ITEM-11); no editor is hand-rolled where a library fits.

## Migration collisions

- **One migration — `132_create_conversation_deliverables.sql`.** `ls migrations/` tail
  is `131`, so `132` is free. Pure link table (no ALTER, no `created_by` vocabulary
  change, no permission grant — reuses file + `conversations::read` gating). Everything
  else reuses existing schema (`file_versions`, the `source_message_id` association).

## OpenAPI regen

- **Required, and it touches types (not only endpoints).** New endpoints (append-version,
  file export, conversation export, deliverables list + pin/unpin), a new `SyncEntity::Deliverable`
  (→ the `sync:deliverable` TS key), and a widened `format` enum all flow through
  `just openapi-regen` into `Api.*` + `types.ts` in BOTH `ui/` and `desktop/ui/`. The
  `emit_ts` golden-parity test gates it; the editors + round-trip are pure frontend.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `restore_version` mirror; the missing user-write primitive; row-lock serializes writers.
- **ITEM-2** — verdict: PASS — generalizes the proven `convert_to_pdf` shape; docx/odt/rtf/html are native pandoc writers.
- **ITEM-3** — verdict: PASS — user download in a chosen format; reuses pandoc + `content_disposition`.
- **ITEM-4** — verdict: CONCERN — new conversation→markdown serializer must faithfully handle every `MessageContentData` variant; bounded, per-variant unit test.
- **ITEM-5** — verdict: CONCERN — deriving deliverables must reuse the `available_files` ownership join or risk a cross-user leak; ownership integration test.
- **ITEM-6** — verdict: CONCERN — heavyweight editor dep: bundle (lazy-load), biome (kit adoption), syncpack, peer pins — all budgeted, none blocking.
- **ITEM-7** — verdict: CONCERN — markdown round-trip fidelity across two engines; mitigated by a constrained subset, verbatim-preserve, normalize-on-save, and a fidelity + render-parity test.
- **ITEM-8** — verdict: CONCERN — edits the shared `file` panel; must gate edit per type and render an arbitrary editable `fileId`; unit predicate + e2e.
- **ITEM-9** — verdict: PASS — literature `tool_result`→`displayInRightPanel` mirror; first-appearance-only auto-open.
- **ITEM-10** — verdict: PASS — small menus in existing header slots.
- **ITEM-11** — verdict: CONCERN — large new component surface (three editors + popovers) must pass design-system + a11y + gallery/state-matrix/testid/kit gates; toolbar accessible names are the main load; its own item + gallery e2e.
- **ITEM-12** — verdict: CONCERN — regen (now type-surface) + desktop mirror + `npm run check` (incl. syncpack) in both workspaces are hard gates.
- **ITEM-13** — verdict: PASS — the tabbed panel already supports multiple files; the dirty guard is additive per-tab UI, no backend.
- **ITEM-14** — verdict: CONCERN — must compare the editor's base version to the incoming `sync:file` head and never auto-clobber; "keep mine" goes through the append path; concurrent-edit e2e.
- **ITEM-15** — verdict: PASS — pure frontend; quotes the selection into the composer; reuses the send path + `available_files`; no mutation.
- **ITEM-16** — verdict: CONCERN — relies on a unique-substring selection for `edit_file(old_str=…)`; a non-unique selection must degrade gracefully; reuses `edit_file`; unit + e2e.
- **ITEM-17** — verdict: CONCERN — migration 132 (verified free) + `SyncEntity::Deliverable` (regen); the derived∪pinned−hidden merge + ownership covered by an integration test.
- **ITEM-18** — verdict: PASS — pin/unpin UI + `sync:deliverable` self-gated refetch mirrors the existing store sync pattern.
- **ITEM-19** — verdict: CONCERN — second editor dep (CodeMirror): same lazy/kit/syncpack/peer mitigations as Plate; plain-text so no round-trip risk.
- **ITEM-20** — verdict: CONCERN — CSV parse↔serialize must be lossless (quoting/embedded delimiters/header); reuse the tabular-viewer parser; round-trip unit test.
- **ITEM-21** — verdict: PASS — reuses `POST /api/files/upload`; the embedded image is a markdown link that survives serialize; enforce existing upload limits.
- **ITEM-22** — verdict: PASS — frontend diff over the existing `versions/{v}/text` endpoint; no backend.
- **ITEM-23** — verdict: CONCERN — widening to odt/rtf/html needs a per-format smoke test to confirm the embedded pandoc emits valid output; per-format export integration test.
