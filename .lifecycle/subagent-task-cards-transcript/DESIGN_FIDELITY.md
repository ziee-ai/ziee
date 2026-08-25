# DESIGN_FIDELITY — subagent-task-cards-transcript

One fidelity verdict per invariant (from PLAN.md `## Invariants`).

- **INV-1** — fidelity: UPHELD — ITEM-1 removes the clip at its ROOT (the flex
  child `overflow-hidden`→`min-height:0` shrink) via `shrink-0`, and ITEM-2 keeps
  the card token-only + kit-only (kit `Card`/`Tag`/`Text`, no raw element, no
  hardcoded color — `npm run check` lint enforces this). The card sizes to its
  content and the panel scrolls, at desktop and 390px (proven by the INV-1
  acceptance e2e asserting the meta row is fully visible in a populated render).
- **INV-2** — fidelity: UPHELD — ITEM-3 surfaces the persisted transcript through
  a named, default-selected **Transcript** tab on the terminal card, reading the
  already-exposed `BackgroundRunDetail.activity` projection of
  `agent_transcript_json`'s agent-loop events. It is discoverable (a labelled tab,
  not buried under the result) and proven by the INV-2 acceptance e2e that opens a
  card and asserts transcript turns render under the Transcript tab.
- **INV-3** — fidelity: UPHELD — ITEM-3 reuses the shared `AgentActivityTimeline`,
  the existing `BackgroundRunResult`, kit `Card`/`Tabs`, and the panel's existing
  pagination/count — mirroring `SubAgentActivityCard`'s transcript drill-in rather
  than reimplementing any of them. The INV-3 acceptance assertion checks the
  reused shared-timeline testid (`wf-activity-timeline-agent`) renders inside the
  card, proving the shared primitive (not a parallel copy) is what draws it.
