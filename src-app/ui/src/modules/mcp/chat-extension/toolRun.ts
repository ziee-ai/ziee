import type {
  MessageContent,
  MessageContentDataToolUse,
  MessageContentDataToolResult,
} from '@/api-client/types'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'

/**
 * Pure helpers for the "N tools called" group card (`McpToolGroupCard`).
 * Extracted so the auto-open policy + artifact detection + artifact
 * attribution are unit-testable without React.
 */

/** The `tool_use` ids present in a run (the producing tool calls). */
export function runToolUseIds(run: MessageContent[]): string[] {
  const ids: string[] = []
  for (const b of run) {
    if (b.content_type !== 'tool_use') continue
    const id = (b.content as MessageContentDataToolUse | undefined)?.id
    if (id) ids.push(id)
  }
  return ids
}

/**
 * True if any `tool_result` in the run carries ≥1 `resource_link` — i.e. the
 * run produced a file artifact that should be visible without a click.
 */
export function hasArtifactInRun(run: MessageContent[]): boolean {
  return run.some(
    b =>
      b.content_type === 'tool_result' &&
      ((b.content as MessageContentDataToolResult | undefined)?.resource_links
        ?.length ?? 0) > 0,
  )
}

/**
 * Whether a tool run is folded into the collapsible `McpToolGroupCard` wrapper.
 * - A run of ≥2 tool calls always wraps (the original "N tools called" group).
 * - A SINGLE tool call wraps too WHEN it produced an artifact, so its file(s) sit
 *   in the same collapsible box (visually consistent with the multi-tool group)
 *   instead of rendering as a bare card with the files loose below it.
 * - A single tool call with NO artifact does NOT wrap (stays the plain card).
 *
 * This is the SINGLE source of truth shared by `McpToolUseGroup` (the render
 * branch) and its `contentSpan` (how many blocks the run-loop consumes). They MUST
 * agree — a group that renders N blocks but reports a different `consumed` corrupts
 * subsequent block rendering — so both call this on the same `run`.
 */
export function shouldWrapRun(run: MessageContent[]): boolean {
  const toolUseCount = runToolUseIds(run).length
  return toolUseCount >= 2 || (toolUseCount >= 1 && hasArtifactInRun(run))
}

/**
 * The default-open (latch) condition: a group opens on its own when a tool is
 * running or it has produced an artifact. This is the initial `userOpen` value
 * AND the effect trigger — once true it latches `userOpen` open; the user may
 * still collapse it afterward (it does not force-open continuously).
 */
export function shouldAutoOpen(args: {
  hasRunning: boolean
  hasArtifact: boolean
}): boolean {
  return args.hasRunning || args.hasArtifact
}

/**
 * The final render decision for a group card.
 * - A `pending_approval` tool FORCES the group open (a collapsed group would
 *   hide the approval prompt and strand the user), overriding a user collapse.
 * - Otherwise the group follows `userOpen` (which starts at `shouldAutoOpen`,
 *   latches open on running/artifact, and is toggled by the user).
 */
export function deriveGroupOpen(args: {
  hasPendingApproval: boolean
  userOpen: boolean
}): boolean {
  return args.hasPendingApproval || args.userOpen
}

/**
 * Resolve which `tool_use` an incoming artifact belongs to, robust under
 * parallel tools.
 * - Prefer the explicit `eventToolUseId` (the current backend always sends it).
 * - Legacy fallback (no event id): attribute ONLY when unambiguous — exactly one
 *   `tool_use` block in the message, or exactly one in-flight
 *   (`started`/`pending_approval`) store call. Otherwise return `null` and skip:
 *   never guess "the last tool_use", which would mis-attach a parallel artifact.
 */
export function resolveArtifactToolUseId(
  contents: MessageContent[],
  storeCalls: ReadonlyMap<string, McpToolCall>,
  eventToolUseId?: string | null,
): string | null {
  if (eventToolUseId) return eventToolUseId

  const toolUseIds = contents
    .filter(c => c.content_type === 'tool_use')
    .map(c => (c.content as MessageContentDataToolUse | undefined)?.id)
    .filter((id): id is string => !!id)
  if (toolUseIds.length === 1) return toolUseIds[0]

  // Disambiguate via a single in-flight call — but only among THIS message's
  // tool_use ids, never the global store: an in-flight call from another
  // conversation (or a prior turn) must not capture this artifact, and the
  // returned id must be a tool_use that actually exists in this message.
  const messageUseIds = new Set(toolUseIds)
  const inFlight = [...storeCalls.values()].filter(
    c =>
      (c.status === 'started' || c.status === 'pending_approval') &&
      messageUseIds.has(c.tool_use_id),
  )
  if (inFlight.length === 1) return inFlight[0].tool_use_id

  return null
}
