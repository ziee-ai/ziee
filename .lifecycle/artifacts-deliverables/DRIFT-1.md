# DRIFT-1 — implementation vs plan (Phase A + Phase B foundation)

Divergences discovered while implementing. **Implementation is IN PROGRESS** — this
round reconciles the plan against what has been built + verified so far (backend
complete; markdown-canvas frontend core built + its data path runtime-verified). The
remaining editors (code/CSV), auto-open, selection→LLM, multi-file safety, pin UI,
gallery, and phases 6–8 are not yet built; those are tracked as remaining work, not as
unresolved drift.

## Reconciled divergences

- **DRIFT-1.1** — verdict: impl-wins — Migration is **133**, not 132. The Phase-2 audit's
  "132 is free" was checked against a stale local `main`; origin/main (which the worktree
  tracks) already had `132_add_openrouter_provider_type`. A duplicate version is silently
  dropped by sqlx, so the plan is amended to 133. (See [[project_migration_numbering]].)

- **DRIFT-1.2** — verdict: impl-wins — Conversation export gates on `MessagesRead`, not
  DEC-18's `conversations::read`. `MessagesRead` is the permission that actually protects
  reading message *content*, which a transcript export exposes — the more-correct gate.

- **DRIFT-1.3** — verdict: resolved — Deliverables list gates `ConversationsRead`, pin/
  unpin `ConversationsEdit` — matches DEC-18's intent (conversation-scoped read/edit).

- **DRIFT-1.4** — verdict: impl-wins — ITEM-16 (conversation export UI) **enhances the
  existing** `chat/extensions/export/extension.tsx` `+`-menu rather than adding a new
  chat-header menu. That extension already existed (client-side JSON/Text/Markdown); it now
  routes md/docx/pdf/odt/rtf/html through the new faithful backend endpoint. Reuse, not
  rebuild.

- **DRIFT-1.5** — verdict: resolved — New file endpoints require an explicit
  `.id("File.x").tag("Files")` in their `_docs` fns or the openapi→ts generator SILENTLY
  omits them (no client method generated). Not captured in the plan; added to all five new
  endpoints. Caught by verifying the generated client, not by compiling.

- **DRIFT-1.6** — verdict: plan-wins — **DEC-7's "unknown constructs preserved verbatim"
  is FALSE for Plate.** Runtime round-trip testing proved Plate *drops* unmodeled
  constructs (lists were silently lost on save until `@platejs/list` was added). The plan's
  intent (no data loss) stands — so the design rule is corrected to: **every supported GFM
  construct MUST have its Plate plugin, or it is dropped.** The full GFM subset
  (headings/marks/lists/tables/links/images/code/blockquote) is now runtime-verified
  (12/12 in `markdownRoundtrip.test.ts`). Amend DECISIONS DEC-7/DEC-8 accordingly.

- **DRIFT-1.7** — verdict: resolved — Editable-type coverage is being delivered
  incrementally: markdown (Plate) is built + verified; code (CodeMirror) and csv (grid)
  editors from DEC-5 are not yet implemented. No plan change — this is remaining work, not
  a divergence.

## Deferred verification (not a drift, a known gap)

The React editor components are `tsc`-clean but NOT browser-render-verified — the gallery
here is conversation-specialized, so component-render verification (ITEM-11) is its own
task. The **data path** (markdown round-trip) IS runtime-verified via `node --test`.

**Unresolved drifts:** 0
