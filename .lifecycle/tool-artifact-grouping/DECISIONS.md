# DECISIONS — tool-artifact-grouping (follow-up)

All inputs resolved up front (design confirmed with the human in plan mode). No
unresolved markers.

### DEC-1: When does a run get wrapped in the collapsible card?
**Resolution:** `shouldWrapRun(run)` = `≥2 tool calls` OR `(≥1 tool call AND hasArtifactInRun)`. A single tool with no artifact stays a plain `McpToolUseRenderer` card.
**Basis:** user — the follow-up task's exact spec ("wrap when a single tool produces an artifact; a single tool with no artifact must still render as the plain card").

### DEC-2: How do `McpToolUseGroup` and `contentSpan` stay in sync?
**Resolution:** Both call the SAME `shouldWrapRun(collectToolRun(blocks, index))`. Single source of truth; no separate thresholds.
**Basis:** codebase/convention — the run-loop desyncs if the render-branch and `contentSpan` disagree (task's explicit warning); one shared predicate makes disagreement impossible.

### DEC-3: Header text for a single-tool wrapper.
**Resolution:** Show the tool name + server label (mirroring the bare `McpToolCallUI`/`McpToolUseRenderer` header). "{n} tools called" is kept only for ≥2 tools.
**Basis:** user/convention — "N tools called" is wrong/uninformative for one call; the task says design it to read sensibly, and reusing the single-tool card's header is the most consistent choice.

### DEC-4: Where does the scroll-to-approval live — group card or approval component?
**Resolution:** The approval component `ToolCallPendingApprovalContent.tsx`.
**Basis:** codebase — it renders in BOTH the lone and grouped approval paths, and only mounts after the group has force-opened, so one effect covers both with no scroll-races-the-expand hazard (the task recommends this and asked to justify; the group-ref alternative needs its own expand-race handling and duplicates logic).

### DEC-5: Scroll behavior + how often it fires.
**Resolution:** `scrollIntoView({ behavior, block: 'nearest' })` on mount; `behavior='auto'` under `prefers-reduced-motion: reduce` else `'smooth'`; fire ONCE per `tool_use_id` via a module-level `Set` (survives the loadMessages remount, not per-render).
**Basis:** convention/accessibility — the task's guardrails (once per approval, respect reduced motion, scroll the ScrollArea not the window via `block:'nearest'`, don't hijack streaming scroll). Precedents: `ConversationFindBar.tsx:146`, `ConversationCard.tsx:55`.

### DEC-6: Configurable-settings rule — any operational tunable introduced?
**Resolution:** No. Pure client-side render/UX logic (a wrap predicate + a scroll-into-view). No limit/retention/rate/toggle/threshold. N/A.
**Basis:** convention — none of the configurable-settings trigger categories apply.

### DEC-7: Update the existing single-tool-artifact e2e specs?
**Resolution:** Yes — reconcile `mcp-resource-links-{positioning,streaming}.spec.ts` to the new wrapped layout (files now render inside the auto-opened group; keep visibility/count assertions, fix bare-card/positioning ones).
**Basis:** codebase — those specs seed single-tool artifacts whose rendering intentionally changes; updating them reflects the new correct behavior (not a workaround).
