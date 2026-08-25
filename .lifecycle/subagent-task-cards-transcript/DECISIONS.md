# DECISIONS — subagent-task-cards-transcript

All resolved up front by convention / the task brief / the design system. No
genuine product choice remained ambiguous enough to require an `AskUserQuestion`
(the two problems are well-specified bug fixes). One candidate refinement is
flagged to the owner at phase 9 (DEC-4).

### DEC-1: Transcript data source — reuse the exposed `activity` field, or add a new backend read for raw `agent_transcript_json`?
**Resolution:** Reuse the already-exposed `BackgroundRunDetail.activity`
(`GET /api/background/runs/{id}`), the server projection of the agent-loop events
also captured in `agent_transcript_json`. NO backend change.
**Basis:** codebase + task brief — the brief says "PREFER an existing read
endpoint/field"; `activity` is already served, already the exact
`AgentActivityTimeline` shape, and VERIFIED populated on real runs (rig: 1–12
entries/run, tracking the transcript message count). Reusing it also avoids
touching `background_mcp` routes/handlers/repository, which another agent is
concurrently editing.

### DEC-2: How is the transcript made discoverable — tabs, two toggle buttons, or one toggle?
**Resolution:** A kit `Tabs variant="line" size="sm"` region on the terminal
card, with a **Transcript** tab and a **Result** tab.
**Basis:** DESIGN_SYSTEM "component variant selection" (a dense/narrow side panel
uses quiet `variant="line"` tabs); mirrors the sibling `SubAgentActivityCard`
transcript drill-in. One labelled region beats two independent inline expanders.

### DEC-3: Which tab is selected by default?
**Resolution:** **Transcript** is the default-selected tab; Result is one click away.
**Basis:** task #3 JTBD — the unmet job is "open a task card and see the agent's
transcript"; the final result was already reachable, the transcript was not.

### DEC-4: Result preview on the COLLAPSED card — eager per-card detail fetch, or on-expand only?
**Resolution:** On-expand only. The collapsed card shows a state summary (kind,
time, tokens, Result-ready / error); the full result + transcript load lazily when
the card is expanded (existing `loadRunDetail`, cached).
**Basis:** convention — the store is already built lazy-detail-per-card, and the
UI-surface scale/cardinality rule forbids fetch-all/render-all; eagerly fetching
every run's detail on panel open is an N-fetch fan-out over the page. A collapsed
one-line text preview would require exactly that fan-out. Flagged to the owner at
phase 9 as a possible future refinement (eager first-line preview) — NOT silently
chosen away, but the convention default ships.

### DEC-5: Clip-fix mechanism — `shrink-0` on the card, remove `overflow-hidden`, or set a min-height?
**Resolution:** Add `shrink-0` (flex-none) to the card in the panel.
**Basis:** codebase — the kit `Card`'s `overflow-hidden` is intentional (rounded
corners + first/last-child image clipping) and must stay; the defect is that a
flex child with `overflow-hidden` gets `min-height:0` and shrinks. `shrink-0` is
the standard fix and lets the panel's existing `overflow-y-auto` own the scroll,
matching sibling panels.

### DEC-6: Title rendering — `line-clamp-2` or single-line `truncate`?
**Resolution:** `line-clamp-2` (wraps to two readable lines, then ellipsizes).
**Basis:** task #2 — the "hard-truncated bold title" is called out as a defect; a
two-line clamp keeps the title readable while bounding height.

### DEC-7: Does this feature introduce any operational tunable (limits / retention / toggles / thresholds)?
**Resolution:** NO. No new settings row / migration / constant.
**Basis:** convention — reuses the existing `PANEL_PAGE_SIZE` (20) pagination
constant and `AgentActivityTimeline`'s `MAX_VISIBLE` (40) head-collapse; neither
is newly introduced here, so the mandatory configurable-settings rule is satisfied
with no new admin setting.
