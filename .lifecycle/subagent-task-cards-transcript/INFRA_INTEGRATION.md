# INFRA_INTEGRATION — subagent-task-cards-transcript

Per-item UX / infra-integration / entity-lifecycle walks (phase 5).

## ITEM-1/2 (card layout + redesign)

- **UX walk**: user opens the Tasks panel → sees N cards, each showing status,
  title, kind, time — none clipped; scrolls the list (panel owns scroll). At 390px
  the title wraps, the pill drops under it, the meta row wraps. No horizontal
  scroll.
- **Infra**: the ONLY coupling is the panel's flex-column scroll ownership
  (`BackgroundRunsPanel` `flex h-full flex-col overflow-y-auto`). `shrink-0` on the
  card restores the intended "cards keep natural height, container scrolls"
  behaviour that the panel comment already assumes. No store / sync / streaming
  interaction changes.
- **Entity-lifecycle**: a run's live status Tag still updates via the store's
  `sync:workflow_run` refetch (running → completed) — local + cross-device both
  covered by the existing store subscription; the card is a pure render of the
  summary, so add/remove/mutate of a run is handled by the panel's list re-render
  (unchanged). No new entity introduced.

## ITEM-3 (Transcript / Result tabs)

- **UX walk**: user clicks "Show details" on a completed card → detail lazily
  loads (Spin) → the Transcript tab shows the step-by-step timeline (default);
  clicking Result shows the final output. Empty transcript → a friendly note (not
  a blank tab). A detail-fetch failure → inline Alert with retry-on-reexpand.
- **Infra**: reuses `BackgroundRuns.loadRunDetail` (dedup + cache +
  `detailErrorByRun`), the shared `AgentActivityTimeline` (its own `MAX_VISIBLE`
  head-collapse + 390px wrapping), and `BackgroundRunResult` (shape-guarded). Tab
  selection is component-local `useState` (per card) — no store, no cross-instance
  leak. The MCP/approval/permission/sync paths are untouched (read-only detail).
- **Entity-lifecycle**: a terminal run's detail (result + transcript) is
  immutable, so the cache-once behaviour is correct; if the run is deleted/lost
  the panel drops the card (list re-render) and the detail cache key is simply
  never read again. Access-loss (no `background::use`) is already gated in
  `loadRunDetail` (`hasPermissionNow`) → the fetch no-ops, the tab shows the Spin
  then nothing sensitive; the whole panel is hidden by the existing negative-perm
  gating.

## ITEM-4/5 (gallery + tests)

- **Infra**: the gallery cassette (`Background.getRun`) is the only seam; adding
  `activity` to a fixture is data-only. The e2e specs SQL-seed real runs (no API
  mock), exercising the real REST detail fetch + the real card render.
