# PLAN_AUDIT — tool-artifact-grouping

Audit of PLAN.md against the actual codebase at base `origin/khoi` (`72cbfeee`).

## Breakage risk

- **ChatMessage run-loop coupling (ITEM-2).** The run-loop (`ChatMessage.tsx:112-131`)
  passes `blocks: bubbleBlocks, index: i` to `chatExtensionRegistry.renderContent`,
  and `McpToolUseGroup.contentSpan` re-derives the run from that SAME array via
  `collectToolRun(blocks, index)`. Applying `normalizeToolResultOrder` to
  `bubbleBlocks` before the loop keeps the render and the `contentSpan` consumption
  in lockstep (both see the normalized array), so no double-render / skipped-block
  risk. Verified `contentSpan` is only consulted when `blocks && index != null`
  (`registry.tsx:824-826`); the group re-renders its members WITHOUT `blocks`, so
  normalization never recurses.
- **Lone-tool path unaffected.** `McpToolUseRenderer` resolves its historical
  `tool_result` by `tool_use_id` from `Stores.Chat.messages` (`extension.tsx:164-169`),
  not from the normalized array, so reordering can't break it. For a lone tool,
  `contentSpan` still returns 1 (`countToolUses < 2`), so the `tool_result` renders
  inline via the file extension exactly as today — just guaranteed adjacent.
- **Reactive store read in the group (ITEM-3).** Adding `const { toolCalls } =
  Stores.McpComposer` at the top of `McpToolGroupCard` is the same proven reactive
  pattern already used by `McpToolUseRenderer` (`extension.tsx:139`). Read
  unconditionally at the top → stable hook order; `useState` + one `useEffect` added,
  both unconditional. No conditional-hooks hazard.
- **`artifactCreated` fallback tightening (ITEM-5).** The primary path
  (`data.tool_use_id`) is unchanged; only the legacy no-id fallback changes from
  "guess last tool_use" to "attribute only if unambiguous, else skip". Worst case a
  legacy backend event with no `tool_use_id` AND multiple in-flight tools drops the
  artifact rather than mis-attaching it — strictly safer, and the current backend
  always sends `tool_use_id` (`SSEChatStreamArtifactCreatedData.tool_use_id`), so the
  fallback is effectively dead for live traffic.
- **Non-tool block order (guardrail).** `normalizeToolResultOrder` only relocates
  `tool_result` blocks to their `tool_use`; text/thinking/image blocks keep their
  relative order. A lone tool whose result previously sat after a text block will now
  render the artifact adjacent to the tool (before that text) — intended and more
  correct (artifact belongs to its tool). Documented as an intended minor side effect.

## Pattern conformance

- **ITEM-1/ITEM-4 pure helpers + `node:test`** mirror `chat/core/utils/estimateMessageHeight.ts`
  (+`.test.ts`) and `mcp/chat-extension/components/elicitationOptions.ts` (+`.test.ts`)
  — pure modules, `node:test`/`node:assert`, run by `npm run test:unit`
  (`node --test src/**/*.test.ts`). Conformant.
- **ITEM-3 reactive read** mirrors `McpToolUseRenderer` (`extension.tsx:137-159`).
  Conformant.
- **ITEM-2 sort pipeline** slots into the existing pure `[...contents].sort(...)`
  → `bubbleBlocks` copy (`ChatMessage.tsx:86-104`); never mutates the store array.
  Conformant with the file's stated ordering discipline.
- **Types**: `MessageContent`/`MessageContentDataToolUse`/`MessageContentDataToolResult`/
  `ResourceLink` all exist in `api-client/types.ts` (3518/3556/3565/4506); `resource_links`
  is on the tool_result (`:3597`). No new types.

## Migration collisions

- **None.** No migration added (highest is `..153`); no DB, no backend. N/A.

## OpenAPI regen

- **None required.** No backend type change, no new route, no `permissions.rs` edit.
  `openapi.json` / `api-client/types.ts` untouched → no `just openapi-regen`, and the
  phase-3/8 frontend gates treat this as UI work driven purely by `src-app/ui/**`
  source (generated files excluded). Desktop UI aliases `../../ui/src` (no override
  edit), so no desktop regen either.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new pure util mirroring `estimateMessageHeight.ts`; pure array transform on generic `MessageContent` fields, no new deps, no caller breakage.
- **ITEM-2** — verdict: PASS — slots into the existing `sequence_order` sort → `bubbleBlocks` pipeline; keeps render + `contentSpan` on the same normalized array; lone-tool path resolves by id and is unaffected.
- **ITEM-3** — verdict: PASS — reactive `Stores.McpComposer.toolCalls` read is the established pattern; unconditional hooks; `isExpanded = hasPendingApproval || userOpen` force-open policy matches the approved design.
- **ITEM-4** — verdict: PASS — extracting `hasArtifactInRun`/`runToolUseIds`/`deriveGroupOpen` as pure fns follows the `elicitationOptions.ts` split-for-testability precedent.
- **ITEM-5** — verdict: PASS — strictly safer than the status quo (skip-on-ambiguous vs guess-last); primary `data.tool_use_id` path unchanged; live backend always supplies the id.
