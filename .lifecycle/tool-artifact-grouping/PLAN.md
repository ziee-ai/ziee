# PLAN — tool-artifact-grouping (follow-up round)

Two user follow-ups on the merged #133 work (single-tool artifact wrapping +
scroll-to-approval). Frontend (React/TS) only.

## Items

- **ITEM-1**: Add a shared pure predicate `shouldWrapRun(run: MessageContent[]): boolean` to `mcp/chat-extension/toolRun.ts` = `runToolUseIds(run).length >= 2 || (runToolUseIds(run).length >= 1 && hasArtifactInRun(run))` — reusing the existing `runToolUseIds` + `hasArtifactInRun`. It decides whether a tool run is wrapped in the collapsible `McpToolGroupCard`: a run of ≥2 tool calls (unchanged) OR a single tool call that produced an artifact (new).
- **ITEM-2**: Route BOTH `McpToolUseGroup(props)` and `McpToolUseGroup.contentSpan` in `extension.tsx` through `shouldWrapRun(collectToolRun(blocks, index))` so they agree exactly (group renders iff `shouldWrapRun`; `contentSpan` returns `run.length` iff `shouldWrapRun` else `1`). This prevents the ChatMessage run-loop `consumed` desync. A single tool with NO artifact still falls through to the plain `McpToolUseRenderer`.
- **ITEM-3**: `McpToolGroupCard` header — when the wrapper holds exactly one `tool_use`, render the tool name (`tool_use.content.name`) + server label (`mcpServerParenLabel(Stores.McpServer.servers.find(s => s.id === server_id)?.display_name)`, via a reactive `const { servers } = Stores.McpServer` read) instead of "N tools called". ≥2 tools keeps "{n} tools called". Auto-open is unchanged (`shouldAutoOpen` opens on `hasArtifact`).
- **ITEM-4**: Smooth-scroll a pending approval into view. In `components/ToolCallPendingApprovalContent.tsx`, attach a ref to the outer `div` and `scrollIntoView({ behavior, block: 'nearest' })` on mount, where `behavior` respects `prefers-reduced-motion` (`'auto'` when reduced, else `'smooth'`). Fire ONCE per approval via a module-level `Set<string>` of already-scrolled `tool_use_id`s (survives the loadMessages remount; not per-render). Covers both the lone-approval and grouped-approval paths (the same component renders in both, only after the group has force-opened). Guard `typeof window` + null ref.
- **ITEM-5**: Reconcile the existing single-tool-artifact e2e specs (`tests/e2e/chat/mcp-resource-links-positioning.spec.ts`, `…-streaming.spec.ts`) which seed single-tool artifacts and now render wrapped — update only assertions that assumed no wrapper (files stay visible/counted since the group auto-opens).

## Files to touch

- EDIT `src-app/ui/src/modules/mcp/chat-extension/toolRun.ts` (+ `shouldWrapRun`)
- EDIT `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts` (+ `shouldWrapRun` cases)
- EDIT `src-app/ui/src/modules/mcp/chat-extension/extension.tsx` (`McpToolUseGroup` + `contentSpan`; `McpToolGroupCard` header)
- EDIT `src-app/ui/src/modules/mcp/chat-extension/components/ToolCallPendingApprovalContent.tsx` (scroll effect)
- ADD `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` (+ optional scroll assertion)
- EDIT `src-app/ui/tests/e2e/chat/mcp-resource-links-positioning.spec.ts` + `…-streaming.spec.ts` (reconcile)
- Optional ADD a `deep-chat-tool-single-artifact` gallery state (`src/dev/gallery/`) for gate:ui coverage.

## Patterns to follow

- **Pure predicate + colocated `node:test`** — `shouldWrapRun` sits beside the existing `hasArtifactInRun`/`runToolUseIds`/`deriveGroupOpen` in `toolRun.ts`, tested in `toolRun.test.ts` (same `node:test` style as #133).
- **Single-tool header** mirrors `McpToolCallUI` (`extension.tsx:43-59`) and `McpToolUseRenderer` (server resolved from `Stores.McpServer.servers` by `server_id` → `mcpServerParenLabel(display_name)`).
- **scrollIntoView** mirrors `ConversationFindBar.tsx:146` (`block: 'nearest'` against the chat `ScrollArea`); `matchMedia` usage mirrors `ConversationCard.tsx:55`.
- **e2e** uses the existing `sse-mock-helpers.ts` + `chat/fixtures/mock-tool-result.ts` (`seedAssistantWithToolResult` is single-tool; `mcpApprovalRequiredEvent` for the approval path); testids `mcp-toolgroup-card`, `mcp-tooluse-card-<id>`, `tool-result-files`, `tool-approval-<id>`.
