import type {
  MessageContent,
  MessageContentDataToolUse,
  MessageContentDataToolResult,
} from '@/api-client/types'

/**
 * Relocate each `tool_result` block to sit immediately after its producing
 * `tool_use` block (matched by `tool_use_id`), preserving the relative order of
 * every other block.
 *
 * A tool call is emitted as a `tool_use` + a matching `tool_result` (the result
 * may carry files as `resource_links`). Depending on streaming timing or the
 * persisted order, an artifact `tool_result` can land far from its `tool_use`
 * (e.g. appended at the end of the message, after a trailing `text` block). The
 * MCP group renderer folds only a CONTIGUOUS run of tool blocks into the
 * "N tools called" card, so a stray `tool_result` renders unwrapped, next to the
 * group. Normalizing the order here makes the run contiguous regardless of a
 * block's physical position, so the grouping/expand logic behaves identically
 * during streaming and after a reload.
 *
 * Rules:
 * - A `tool_result` with a matching `tool_use` (by id) is emitted immediately
 *   after that `tool_use`; multiple results for one use keep their relative order.
 * - A `tool_result` with no matching `tool_use` (orphan) stays in place.
 * - Non-tool blocks (text / thinking / image / …) keep their relative order.
 * - Pure: the input array and its elements are never mutated; the returned array
 *   holds the same element references. Idempotent (a normalized array is
 *   returned unchanged in order).
 */
export function normalizeToolResultOrder(
  blocks: MessageContent[],
): MessageContent[] {
  // Index tool_result blocks by their tool_use_id, preserving encounter order
  // (a tool may return more than one result block over its lifetime).
  const resultsByUseId = new Map<string, MessageContent[]>()
  for (const b of blocks) {
    if (b.content_type !== 'tool_result') continue
    const useId = (b.content as MessageContentDataToolResult).tool_use_id
    if (!useId) continue
    const list = resultsByUseId.get(useId)
    if (list) list.push(b)
    else resultsByUseId.set(useId, [b])
  }
  // No matched results possible without both a tool_use id set and a result — a
  // cheap exit that also guarantees identity for the common non-tool message.
  if (resultsByUseId.size === 0) return blocks

  // The set of tool_use ids actually present. An orphan result (whose id is NOT
  // here) must be left in its original slot rather than hoisted.
  const presentUseIds = new Set<string>()
  for (const b of blocks) {
    if (b.content_type !== 'tool_use') continue
    const id = (b.content as MessageContentDataToolUse).id
    if (id) presentUseIds.add(id)
  }

  const emitted = new Set<MessageContent>()
  const out: MessageContent[] = []
  for (const b of blocks) {
    if (emitted.has(b)) continue
    if (b.content_type === 'tool_result') {
      const useId = (b.content as MessageContentDataToolResult).tool_use_id
      // A matched result is emitted right after its tool_use (below); skip it in
      // its original slot so it isn't rendered twice / out of place. An orphan
      // result (no matching tool_use present) falls through and emits here.
      if (useId && presentUseIds.has(useId)) continue
    }
    out.push(b)
    emitted.add(b)
    if (b.content_type === 'tool_use') {
      const id = (b.content as MessageContentDataToolUse).id
      const results = id ? resultsByUseId.get(id) : undefined
      if (results) {
        for (const r of results) {
          if (emitted.has(r)) continue
          out.push(r)
          emitted.add(r)
        }
      }
    }
  }
  // Defensive: never drop a block. Every skipped result maps to a present
  // tool_use, so this should be a no-op — but it guarantees the output is a
  // permutation of the input even if that invariant ever changes.
  for (const b of blocks) {
    if (emitted.has(b)) continue
    out.push(b)
    emitted.add(b)
  }
  return out
}
