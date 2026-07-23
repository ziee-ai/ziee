import type { AgentActivityEntry } from '@/modules/workflow/components/run/activityDescriptors'

/** Cap on the most-recent agent-activity rows retained per step in the client
 *  store. Mirrors the backend `AGENT_ACTIVITY_MAX_ENTRIES` (repository.rs) so a
 *  long run can't grow this array without bound. When exceeded we drop the
 *  lowest-`seq` (oldest) rows, matching the backend's chronological trim. */
export const AGENT_ACTIVITY_MAX_ENTRIES = 500

/** Trim the (ascending-by-seq) list in place to the most-recent
 *  `AGENT_ACTIVITY_MAX_ENTRIES`, dropping the lowest-seq head. Bounded work
 *  (≤ overflow count), so it keeps per-frame merges O(1) amortized. */
function trimActivity(list: AgentActivityEntry[]) {
  if (list.length > AGENT_ACTIVITY_MAX_ENTRIES) {
    list.splice(0, list.length - AGENT_ACTIVITY_MAX_ENTRIES)
  }
}

/** Merge one agent-activity payload into an ordered, seq-deduped list (ascending
 *  by `seq`). Re-emitting the same seq (e.g. a `running`→`ok` status upgrade)
 *  REPLACES the existing row in place rather than appending a duplicate.
 *
 *  O(1) amortized: `seq` is monotonic and frames almost always arrive in order,
 *  so the common cases (new tail / tail status-upgrade) are constant time; only
 *  a genuinely out-of-order straggler pays an O(n) scan. The stored array is
 *  then capped so memory can't grow unbounded over a long run. */
export function mergeAgentActivity(list: AgentActivityEntry[], entry: AgentActivityEntry) {
  const n = list.length
  if (n === 0) {
    list.push(entry)
  } else {
    const last = list[n - 1]
    if (entry.seq > last.seq) {
      // Common case: strictly newer → append (O(1)).
      list.push(entry)
    } else if (entry.seq === last.seq) {
      // Common case: status upgrade on the newest row → replace tail (O(1)).
      list[n - 1] = entry
    } else {
      // Rare: out-of-order seq. Locate the first row ≥ entry.seq and either
      // replace (dedupe) or splice-insert to preserve ascending order.
      const i = list.findIndex(e => e.seq >= entry.seq)
      if (i >= 0 && list[i].seq === entry.seq) list[i] = entry
      else if (i >= 0) list.splice(i, 0, entry)
      else list.push(entry)
    }
  }
  trimActivity(list)
}

/** Bulk-merge a persisted activity array into `list` in O(n + m) — one seq→index
 *  map, in-place replace for existing seqs, append + a single sort for the new
 *  ones — instead of an O(n²) per-element `findIndex`. Used by snapshot
 *  rehydrate, where the persisted array can be large. */
export function mergeAgentActivityBatch(
  list: AgentActivityEntry[],
  incoming: AgentActivityEntry[],
) {
  if (incoming.length === 0) return
  const idxBySeq = new Map<number, number>()
  list.forEach((e, i) => idxBySeq.set(e.seq, i))
  let appended = false
  for (const e of incoming) {
    const i = idxBySeq.get(e.seq)
    if (i !== undefined) {
      list[i] = e // replace in place (dedupe / status upgrade)
    } else {
      list.push(e)
      appended = true
    }
  }
  // Persisted rows are chronological, but re-sort once only if we appended, to
  // restore the ascending-seq invariant defensively.
  if (appended) list.sort((a, b) => a.seq - b.seq)
  trimActivity(list)
}
