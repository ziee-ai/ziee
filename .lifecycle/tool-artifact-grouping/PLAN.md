# PLAN — tool-artifact-grouping

Fix three defects in the ziee chat UI's MCP tool-call rendering: (1) MCP-server
artifacts escape the "N tools called" group box (grouping is positional, not by
`tool_use_id`); (2) a ≥2-tool group never auto-opens for a `pending_approval` /
`running` tool, hiding the approval prompt; (3) a group never auto-opens for an
artifact. Frontend (React/TS) only.

## Items

- **ITEM-1**: Add a pure, generic helper `normalizeToolResultOrder(blocks: MessageContent[]): MessageContent[]` in `modules/chat/core/utils/normalizeToolResultOrder.ts` that relocates each `tool_result` block to sit immediately after its producing `tool_use` block (matched by `tool_use.content.id === tool_result.content.tool_use_id`), preserving every non-tool block's relative order and leaving orphan `tool_result`s (no matching `tool_use`) in place. This makes a tool run contiguous regardless of where an artifact `tool_result` physically lands (streaming-appended-at-end OR reload/persisted order).
- **ITEM-2**: Wire `normalizeToolResultOrder` into `ChatMessage.tsx` — apply it to `bubbleBlocks` after the existing `sequence_order` sort and before the render run-loop, so `collectToolRun` / `contentSpan` / `McpToolGroupCard` (all unchanged) see a contiguous run and wrap the artifact `tool_result`. Must not reorder non-tool blocks relative to each other; must not regress the lone-tool inline-artifact path.
- **ITEM-3**: Make `McpToolGroupCard` auto-open. Read live tool statuses reactively from `Stores.McpComposer.toolCalls` (Map keyed by `tool_use_id`; status ∈ `started|pending_approval|completed|error`). Derive `hasPendingApproval`/`hasRunning` from the run's `tool_use` ids and `hasArtifact` from any run `tool_result` carrying `resource_links`. Open state: `userOpen` initializes to `hasRunning || hasArtifact` with a reactive effect that opens it when those become true after mount; `isExpanded = hasPendingApproval || userOpen` so a pending approval FORCES the group open (cannot be collapsed to hide the prompt), while running/artifact merely default-open and stay user-collapsible.
- **ITEM-4**: Extract the auto-open decision + artifact detection into pure helpers in `modules/mcp/chat-extension/toolRun.ts` — `hasArtifactInRun(run)`, `runToolUseIds(run)`, `shouldAutoOpen({ hasRunning, hasArtifact })` (the latch/default-open trigger), and `deriveGroupOpen({ hasPendingApproval, userOpen })` (the render decision; pending approval forces open, else follows `userOpen`) — so the open policy is unit-testable without React. `McpToolGroupCard` consumes them. (The decision is split into two functions rather than a single 4-arg predicate so that running/artifact only *default-open* via the latched `userOpen` and stay user-collapsible, while pending-approval force-opens continuously — a single OR of all four would wrongly block collapse during a running tool.)
- **ITEM-5**: Tighten the `artifactCreated` SSE handler's `tool_use_id` resolution in `extension.tsx` to prevent misattribution under parallel tools. Keep `data.tool_use_id` as the primary source. Replace the "last tool_use block" fallback with a pure `resolveArtifactToolUseId(contents, storeCalls, eventToolUseId)` (in `toolRun.ts`) that returns the event id when present, else attributes ONLY when there is exactly one unambiguous candidate (a single `tool_use` in the message, or a single in-flight `started`/`pending_approval` call in the store), else returns null (skip — never guess "last").

## Files to touch

- ADD `src-app/ui/src/modules/chat/core/utils/normalizeToolResultOrder.ts`
- ADD `src-app/ui/src/modules/chat/core/utils/normalizeToolResultOrder.test.ts`
- ADD `src-app/ui/src/modules/mcp/chat-extension/toolRun.ts`
- ADD `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts`
- EDIT `src-app/ui/src/modules/chat/components/ChatMessage.tsx`
- EDIT `src-app/ui/src/modules/mcp/chat-extension/extension.tsx`
- ADD an e2e spec under `src-app/ui/tests/e2e/07-mcp/` (tool-group grouping +
  auto-open), plus a gallery state under `src-app/ui/src/dev/gallery/` if one is
  needed to render the states deterministically (finalized in TESTS.md).

## Patterns to follow

- **Pure util + colocated `node:test`** — mirror `modules/chat/core/utils/estimateMessageHeight.ts` + `estimateMessageHeight.test.ts` and `footnoteScope.ts` + `.test.ts` (pure function module, `import { test } from 'node:test'` + `node:assert/strict`). The mcp helper + test mirror `mcp/chat-extension/components/elicitationOptions.ts` + `elicitationOptions.test.ts`.
- **Reactive store read in a content renderer** — mirror `McpToolUseRenderer` in `mcp/chat-extension/extension.tsx:137-159`: `const { toolCalls } = Stores.McpComposer` (destructure for a reactive subscription), then `toolCalls.get(id)`. Do NOT use the non-reactive `getToolCall()`.
- **Block ordering in ChatMessage** — the normalization slots into the existing `[...contents].sort((a,b) => a.sequence_order - b.sequence_order)` → `bubbleBlocks` pipeline (`ChatMessage.tsx:86-104`), a pure copy that never mutates the store array.
- **Group card structure** — keep `McpToolGroupCard`'s existing Card/Button/ChevronDown markup, `data-testid="mcp-toolgroup-card"` / `mcp-toolgroup-details-btn`, and the icon/`allDone` logic; only change the open-state derivation.
- **E2E for chat/mcp rendering** — mirror the closest existing `tests/e2e/07-mcp/` spec and the gallery mock-cassette approach for deterministic (no-LLM) message rendering (exact fixture chosen in TESTS.md).
