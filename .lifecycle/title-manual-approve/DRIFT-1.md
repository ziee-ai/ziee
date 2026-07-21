# DRIFT-1 — plan vs. implementation

Every point where the implementation diverged from PLAN.md, and which side won.

- **DRIFT-1.1** — verdict: plan-wins — ITEM-7 was planned as frontend-only. Phase-2
  exploration proved the sidebar renders from `ConversationResponse[]` with no message
  text on the wire, so the plan was AMENDED (with the lead's approval) to add ITEM-10,
  the backend `first_message_preview` field. The plan changed rather than the item being
  quietly shipped degraded.

- **DRIFT-1.2** — verdict: impl-wins — the plan said all 9 fallback sites adopt
  `conversationDisplayLabel`. Implementation kept `TitleEditor`'s header on the
  placeholder (routed through the shared constant, without the preview fallback), because
  that header IS the edit affordance and a derived label there implies a title exists.
  Recorded as DEC-19 rather than silently deviating.

- **DRIFT-1.3** — verdict: impl-wins — the plan did not mention the project-conversations
  query. The infra walk found `ProjectConversationsList` reuses the SAME `ConversationCard`,
  so populating the preview only on `/conversations` would have shown "Untitled" for project
  rows while the sidebar showed a preview. The same projection was added there.

- **DRIFT-1.4** — verdict: resolved — ITEM-5 / ITEM-6 (the gpt-oss routing fix and the loop
  terminator) were planned as code changes. The live repro showed prefix-less `query_rag`
  ALREADY resolves correctly, so there was no defect to fix. Both descoped under DEC-1's
  pre-approved split gate, with dispositions recorded in DECISIONS.md; ITEM-4 still shipped
  as a diagnostics improvement.

- **DRIFT-1.5** — verdict: plan-wins — the plan's TEST-6 claimed a strict SSE-ordering
  guarantee. Implementation found the driver multiplexes two channels through one
  `tokio::select!`, so no such ordering exists. The ASSERTION was dropped (the flaky test
  removed) rather than the architecture changed to satisfy a test.

**Unresolved drifts:** 0
