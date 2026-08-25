# PLAN — subagent-task-cards-transcript

Fix the chat right-panel "Tasks" (background sub-agent runs) surface: the task
cards render visually broken (clipped), and the persisted agent transcript is not
a discoverable affordance. Pure frontend (`src-app/ui` only); no backend / perm /
migration / API change.

## Design source

- Realizes the repo design contract `agent-kit/docs/DESIGN_SYSTEM.md` (semantic
  color tokens, 4px spacing rhythm, radius scale, `Card`/`Field`/`SectionHeader`
  conventions, the component-variant-selection rule "quiet `variant="line"` Tabs
  in dense/narrow containers", forbidden hardcoded colors / raw elements).
- Realizes the `feature-lifecycle` **UI-surface plan checklist** (precedent,
  scale/cardinality, responsive 390px, populated-render review, JTBD).
- Sibling precedent this surface is the twin of:
  `src-app/ui/src/modules/chat/components/agent-activity/SubAgentActivityCard.tsx`
  (leading kind icon + clamped title + inline status + a lazily-expanded
  `AgentActivityTimeline` transcript) and the existing inline result renderer
  `BackgroundRunResult.tsx`.
- Existing exposed field the transcript is read from: `BackgroundRunDetail.activity`
  (`GET /api/background/runs/{id}`), projected server-side from
  `step_logs_json['agent::agent_activity']` — VERIFIED populated for real runs
  (1–12 entries/run on the live rig, tracking `agent_transcript_json`'s message
  count). No new backend read is required (see DEC-1).

## Invariants

- **INV-1**: cards consume semantic tokens + kit primitives only (no raw hex /
  `bg-*-500`, no hand-rolled `<button>/<input>`), and NO card content (esp. the
  meta row) is clipped/overflow-hidden — the card sizes to its content at desktop
  AND 390px.
- **INV-2**: a completed sub-agent run's full transcript (`agent_transcript_json`,
  surfaced via the `BackgroundRunDetail.activity` projection) is viewable from its
  task card through a discoverable, named **Transcript** view — not only the final
  result.
- **INV-3**: reuse the closest existing card/list/detail primitives + slots
  (affordance-parity) rather than a bespoke reimplementation; mirror the nearest
  sibling surface (`SubAgentActivityCard` transcript drill-in + kit `Card`/`Tabs`
  + the panel's existing `Load more`/count), never a parallel re-implementation.

## Items

- **ITEM-1**: Fix the card clipping. The kit `Card` sets `overflow-hidden`, which
  as a flex child computes `min-height:0`, so in the Tasks panel's `flex h-full
  flex-col` column the N cards SHRINK (measured offsetHeight 58 vs scrollHeight
  152) and clip the meta row + actions instead of the container scrolling. Give
  each card `shrink-0` so it keeps its natural height and the panel's
  `overflow-y-auto` scrolls. No content clipped at desktop or 390px.
- **ITEM-2**: Redesign the card header + meta to a clean, token-only, kit-only
  layout mirroring `SubAgentActivityCard`: a leading kind icon + a readable
  title (`line-clamp-2`, not a hard single-line truncate that reads as clipped) +
  the status `Tag` inline (`ms-auto`); one compact muted meta row (kind label ·
  relative time · tokens · Result-ready). Remove the heavy full-width status row.
- **ITEM-3**: Make the transcript a discoverable, named view. For a terminal run,
  the card expands into a kit `Tabs variant="line" size="sm"` region with a
  **Transcript** tab (default) rendering the shared `AgentActivityTimeline` from
  `detail.activity` (empty → a friendly "No transcript recorded" note) and a
  **Result** tab rendering the existing `BackgroundRunResult`. Detail is
  lazily fetched on first expand and cached (existing `loadRunDetail`). This
  replaces the current single "View result" toggle that buries the timeline
  beneath the result with no "Transcript" affordance.
- **ITEM-4**: Gallery coverage for the new state. Seed `activity` transcript
  entries into `RUN_DETAILS` (`gallery.tsx`) so the Transcript tab renders with
  real data; keep loaded / empty / error states covered; the populated
  transcript+result state is reviewable at desktop and 390px (state-matrix gate).
- **ITEM-5**: Update the existing background e2e + store tests to the new
  affordances WITHOUT losing coverage: `background-transcript.spec.ts` drives the
  new Transcript tab (and the no-activity empty note), and the
  negative-perm / in-conversation / sandbox-panel specs stay green against the
  redesigned card.

## Files to touch

- `src-app/ui/src/modules/background/components/BackgroundRunCard.tsx` (ITEM-1/2/3)
- `src-app/ui/src/modules/background/gallery.tsx` (ITEM-4)
- `src-app/ui/tests/e2e/15-background/background-transcript.spec.ts` (ITEM-5)
- `src-app/ui/tests/e2e/15-background/background-card-layout.spec.ts` (NEW — INV-1 acceptance)
- (read-only mirror check) `src-app/ui/src/modules/background/components/BackgroundRunsPanel.tsx`,
  `BackgroundRunResult.tsx`,
  `src-app/ui/src/modules/workflow/components/run/AgentActivityTimeline.tsx`

## Patterns to follow

- **Card / transcript drill-in** → mirror
  `chat/components/agent-activity/SubAgentActivityCard.tsx` (icon + clamped title
  + inline status; lazy-expand → `AgentActivityTimeline`).
- **Tabs in a dense side panel** → kit `Tabs variant="line" size="sm"` per
  DESIGN_SYSTEM "component variant selection"; triggers/panels get the kit's
  derived `${testid}-tab-<key>` / `${testid}-panel-<key>`.
- **Result rendering** → unchanged `BackgroundRunResult.tsx` (shape-guarded).
- **Panel list / pagination / count** → unchanged `BackgroundRunsPanel.tsx`
  (`Load more`, "Showing N of M").
- **Gallery cassette** → follow the existing `gallery.tsx` shape (keyed
  `Background.getRun` by run id; conversation-aware `listRuns`).

## UI-surface plan checklist (per the redesigned card + its detail region)

- **Precedent**: twin of `SubAgentActivityCard` (transcript drill-in) + kit
  `Card`; the detail region's tabs follow the design-system dense-panel Tabs rule.
  Divergence from that sibling is a bug, not a variant.
- **Scale / cardinality**: the panel already server-paginates (`PANEL_PAGE_SIZE`
  = 20) with `Load more` + "Showing N of M"; the card's transcript is bounded by
  `AgentActivityTimeline`'s `MAX_VISIBLE=40` head-collapse. No fetch-all: detail
  (incl. transcript) is fetched lazily per card on expand and cached — the card
  does NOT eagerly fetch every run's detail on panel open (avoids an N-fetch
  fan-out over the page; DEC-1).
- **Device size / responsive**: at 390px the header title wraps (`line-clamp-2`),
  the status pill drops under it (`flex-wrap` + `ms-auto`), the meta row wraps,
  the `line` Tabs strip stays a single quiet row, and the transcript rows already
  wrap long tokens (`AgentActivityTimeline` `[overflow-wrap:anywhere]`). No
  horizontal page scroll; mirrors the panel's existing `overflow-x-hidden`.
- **Populated-render review**: the gallery seeds a completed sub-agent card WITH
  a transcript + result, reviewed at desktop and 390px (not just empty/error).
- **User-visible progress**: a running run keeps its live status Tag (SSE-driven
  via the store's `sync:workflow_run` refetch); a terminal run shows its result +
  transcript on expand.
- **Input economy**: no new inputs; the steer composer is unchanged.
- **JTBD**: see `## JTBD` below.
- **Multi-instance**: the panel is already per-conversation-keyed in the store;
  this change adds no cross-instance state (tab selection is component-local
  `useState`, per card).
- **Platform-provided affordances**: none added; no in-app chrome duplicating the
  browser.

## JTBD (jobs-to-be-done)

A user watching background sub-agent work in a conversation wants to, per surface:
- **Tasks panel (list)**: scan every task's state at a glance — see its title,
  what kind it is, when it started, and whether it finished — WITHOUT any card
  clipping the very row that says "done / result ready". (Today the meta row is
  cut off on every card.)
- **A task card (detail)**: open one finished task and (a) read the agent's
  step-by-step **transcript** — what it thought, which tools it called, what it
  concluded — as a first-class, obviously-labelled view, and (b) read the final
  **result**. Today the transcript is technically rendered but buried under the
  result with no "Transcript" label, so users don't find it (problem #3).
- **A running task**: steer or cancel it (unchanged).
- **Empty / error / loading**: unchanged, already handled by the panel.

## Plan audit (against the codebase)

Verified each item against the current tree before writing code.

- **ITEM-1** — verdict: PASS — root cause confirmed live: kit `Card`
  (`sdk/packages/kit/src/shadcn/card.tsx`) sets `overflow-hidden`; a live probe on
  the rig measured the card `overflow:hidden flexShrink:1 offsetHeight:58
  scrollHeight:152` (clipped). `shrink-0` is the standard flex-child fix and does
  not change the panel's already-correct `overflow-y-auto` scroll ownership. No
  caller breaks.
- **ITEM-2** — verdict: PASS — mirrors `SubAgentActivityCard` (icon + clamped
  title + inline status). Tokens/kit only; `line-clamp-2` is a Tailwind utility
  already used in the codebase. No new dependency.
- **ITEM-3** — verdict: PASS — kit `Tabs` exists
  (`sdk/packages/kit/src/kit/tabs.tsx`) with `variant="line"`, `size="sm"`, and
  derives `${testid}-tab-<key>` / `${testid}-panel-<key>`. `detail.activity`
  (`ProgressKind[]`) → `AgentActivityEntry[]` filter is the SAME shape the current
  card already renders, so the transcript source is proven. No backend change.
- **ITEM-4** — verdict: PASS — `gallery.tsx` `RUN_DETAILS` is the existing seam;
  adding an `activity` array to a completed run is data-only. `check:state-matrix`
  covers the new populated state.
- **ITEM-5** — verdict: CONCERN — `background-transcript.spec.ts` currently drives
  `background-run-result-toggle-${id}` and expects the timeline inside the result
  panel; it MUST be updated to the new Transcript tab or it fails. Resolved by
  ITEM-5 (update the spec, preserving both its legs — populated transcript +
  no-activity empty state). Other 15-background specs use
  `background-run-card-*` / `background-run-status-*` / `background-run-kind-*` /
  `background-run-steer-*` / `background-panel-*`, all of which the redesign keeps.

## Breakage risk

- Existing e2e specs that reference `background-run-result-toggle-${id}` /
  `background-run-result-panel-${id}` / `background-run-final-text-${id}`: only
  `background-transcript.spec.ts` uses the toggle; it is updated in ITEM-5. The
  final-text/result-panel testids are PRESERVED (they move under the Result tab).
- Store API unchanged (`loadRunDetail`, `detailsByRun`, `detailErrorByRun` reused)
  → `BackgroundRuns.store.test.ts` unaffected.
- No runtime/type contract change; no consumer outside the background module.

## Pattern conformance

- Card structure mirrors `SubAgentActivityCard`; Tabs follow the DESIGN_SYSTEM
  dense-panel rule; result rendering + panel pagination unchanged. No antd import,
  no raw element, no hardcoded color (lint-enforced by `npm run check`).

## Migration collisions

- None. No migration added (frontend-only). Highest server migration
  `202608250100` is untouched.

## OpenAPI regen

- Not required. No handler/type/route change; `openapi.json` +
  `api-client/types.ts` unchanged. `BackgroundRunDetail.activity` already exists.
