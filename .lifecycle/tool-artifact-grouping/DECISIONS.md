# DECISIONS — tool-artifact-grouping

All product/implementation inputs are resolved up front — the design choices
were confirmed with the human in plan mode. No unresolved markers remain.

### DEC-1: Fix Issue 1 by re-sequencing the store, or by render-time normalization?
**Resolution:** Render-time normalization — a pure `normalizeToolResultOrder(blocks)` applied in `ChatMessage.tsx` after the `sequence_order` sort, relocating each `tool_result` adjacent to its producing `tool_use` by `tool_use_id`. No store mutation, no `sequence_order` renumbering.
**Basis:** user — approved in plan mode (recommended option); also codebase — the run-loop only consumes a contiguous span, and normalization is the position-independent fix that works for streaming AND reload without touching the store.

### DEC-2: Open-state policy for the group card.
**Resolution:** `isExpanded = hasPendingApproval || userOpen`; `userOpen` initializes to `hasRunning || hasArtifact` with a reactive `useEffect` that opens it when those become true post-mount. A `pending_approval` tool FORCES the group open (cannot be collapsed to hide the prompt); `running`/artifact merely default-open and stay user-collapsible.
**Basis:** user — approved in plan mode (recommended option). Directly fixes the stuck-user bug (hidden approval) while respecting user toggle for informational states.

### DEC-3: Which tool status literal maps to "running"?
**Resolution:** `'started'`. The group derives `hasRunning` from `Stores.McpComposer.toolCalls.get(id)?.status === 'started'`.
**Basis:** codebase — `McpToolCall.status` union is `'started' | 'pending_approval' | 'completed' | 'error'` (`McpComposer.store.ts:35-48`); there is no `'running'` literal (that string only appears as a presentational `ToolStatusIcon` prop).

### DEC-4: `artifactCreated` misattribution fallback when the event carries no `tool_use_id`.
**Resolution:** Attribute ONLY when unambiguous — exactly one `tool_use` in the message OR exactly one in-flight (`started`/`pending_approval`) store call. Otherwise return null and skip (drop the artifact rather than mis-attach). The primary `data.tool_use_id` path is unchanged.
**Basis:** convention/codebase — safer than the current "guess the last tool_use"; the live backend always sends `tool_use_id` (`SSEChatStreamArtifactCreatedData.tool_use_id`), so this only affects a hypothetical legacy/edge event, where a wrong attachment is worse than none.

### DEC-5: Where do the new pure helpers live + how are they tested?
**Resolution:** `normalizeToolResultOrder.ts` in `modules/chat/core/utils/` (generic block-ordering, belongs with ChatMessage's ordering concern); `toolRun.ts` in `modules/mcp/chat-extension/` (MCP-specific: `hasArtifactInRun`, `runToolUseIds`, `deriveGroupOpen`, `resolveArtifactToolUseId`). Both tested with colocated `node:test` `.test.ts` run by `npm run test:unit`.
**Basis:** codebase — mirrors `estimateMessageHeight.ts`(+`.test.ts`) and `elicitationOptions.ts`(+`.test.ts`).

### DEC-6: Configurable-settings rule — does this feature introduce any operational tunable?
**Resolution:** No. This is pure client-side render/UX logic (block ordering + a boolean open-state derivation). There is no resource limit, retention, rate/quota, concurrency cap, threshold, toggle, or model/provider selection — nothing an operator would tune. No settings row, migration, permission, or admin card. N/A by nature, not by omission.
**Basis:** convention — the mandatory configurable-settings DEC; none of its trigger categories apply to a frontend grouping/expand change.

### DEC-7: Do the desktop UI or backend need parallel edits?
**Resolution:** No. `src-app/desktop/ui` aliases `../../ui/src` via `fallbackSrc` (no hand-written override of the touched files); no backend/OpenAPI change. Only the `src-app/ui` workspace is touched.
**Basis:** codebase — `src-app/desktop/ui/vite.config.ts` `fallbackSrc`/alias to `../../ui/src`; confirmed no duplicate `ChatMessage.tsx`/`extension.tsx` under desktop.

### DEC-8: e2e determinism — how is a tool group rendered without an LLM?
**Resolution:** Use the existing `page.route` SSE/messages mocks (`sse-mock-helpers.ts` + `chat/fixtures/mock-tool-result.ts`). For TEST-4 build a custom persisted message with a non-adjacent artifact `tool_result` via `mockToolUseContent`/`mockToolResultContent` + `mockGetMessages`; for TEST-5 drive a streaming run with `mcpToolStartEvent` + `mcpApprovalRequiredEvent`; for TEST-6 a fully-completed 2-tool run.
**Basis:** codebase — the proven pattern in `mcp-resource-links-positioning.spec.ts` / `mcp-resource-links-streaming.spec.ts`.
