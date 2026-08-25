import { ApiClient } from '@/api-client'
import { defineExtensionStore } from '@/modules/chat/core/extensions'
import type { SubAgentActivityVM } from '@/modules/chat/components/agent-activity/agentActivity'
import type { AgentActivityEntry } from '@/modules/workflow/components/run/activityDescriptors'

/**
 * SubAgentActivityStore — delegated sub-agent activity state (Group A / ITEM-4 /
 * DEC-65 live), now with the ITEM-9 durable per-child transcript drill-in.
 *
 * ## Live child list (SSE, ephemeral)
 * The `subAgentActivity` SSE frame carries `{ run_id, children }` with NO
 * message_id, so the handler keys each snapshot to the IN-FLIGHT assistant
 * message id (learned from the started/content frames). Frames are idempotent
 * FULL snapshots → last-wins (a plain replace, never a merge), so `byMessage`
 * just holds the latest activity snapshot per assistant message.
 *
 * One instance per pane (injected as `Stores.Chat.SubAgentActivityStore`), so a
 * split pane's sub-agent card tracks ITS own stream. Read reactively in the
 * `message_footer` slot via the pane-correct `Stores.Chat` bridge (mirrors the
 * TaskListStore / VoiceStore pattern).
 *
 * `byMessage` is EPHEMERAL BY DESIGN — non-durable across reload. It is fed ONLY
 * by live (non-replay) `subAgentActivity` SSE frames; there is no REST source and
 * no mount/reconnect refetch for the live child *list*, so a page reload mid-run
 * drops the in-memory snapshot and the card's live rows repopulate on the NEXT
 * frame. This is the accepted limitation of the SSE-only live-card design (the
 * whole agent path is default-off, `ZIEE_CHAT_AGENT_CORE`-gated). Do NOT paper
 * over it with a fake persistence layer for the live list.
 *
 * ## Durable per-child transcript (REST, ITEM-9 — the drill-in)
 * The after-the-fact TRANSCRIPT of each child IS durable and now fetchable on
 * demand: `childDetailsById` caches the full agent-loop transcript for a child,
 * lazily loaded via `GET /api/subagent-runs/{child_id}` (`ApiClient.SubAgentRuns.get`,
 * the same `BackgroundRunDetail` shape the background-run detail returns) when the
 * user expands a child row. This closes the exact gap the old comment named — a
 * "durable view" of the sub-agents — without disturbing the ephemeral live list:
 * the row's status badge stays SSE-driven, and the drill-in only adds the
 * historical transcript beside it. A child whose transcript 404s (parent/child
 * pruned after retention / conversation delete) resolves to `{status:'error'}` so
 * the card shows the live status only and never crashes.
 */

/** Bound the retained snapshots so a long session can't grow the map without
 *  limit — sub-agent activity is an ephemeral live overlay; only the newest
 *  turns matter. JS objects preserve insertion order, so the oldest keys evict
 *  first. */
const MAX_TRACKED_MESSAGES = 100

/**
 * A child sub-agent's lazily-fetched transcript state (ITEM-9). `loading` while
 * the REST call is in flight; `loaded` carries the filtered agent-activity
 * transcript + the run's terminal status; `error` on a 404/any failure (a pruned
 * or otherwise unreachable child) so the card degrades to status-only.
 */
export type ChildTranscriptState =
  | { status: 'loading' }
  | { status: 'loaded'; activity: AgentActivityEntry[]; runStatus: string }
  | { status: 'error' }

export const createSubAgentActivityStore = defineExtensionStore({
  immer: false,
  state: {
    /** assistantMessageId → its latest full sub-agent activity snapshot. */
    byMessage: {} as Record<string, SubAgentActivityVM>,
    /** childRunId → its lazily-fetched durable transcript state (ITEM-9). */
    childDetailsById: {} as Record<string, ChildTranscriptState>,
  },
  actions: (set, get) => ({
    /** Replace (last-wins) the snapshot for one assistant message. */
    setForMessage: (messageId: string, activity: SubAgentActivityVM) =>
      set(state => {
        const next: Record<string, SubAgentActivityVM> = {
          ...state.byMessage,
          [messageId]: activity,
        }
        const keys = Object.keys(next)
        if (keys.length > MAX_TRACKED_MESSAGES) {
          for (const stale of keys.slice(0, keys.length - MAX_TRACKED_MESSAGES)) {
            delete next[stale]
          }
        }
        return { byMessage: next }
      }),

    /**
     * Lazily fetch one child sub-agent's durable transcript (`GET
     * /api/subagent-runs/{child_id}`), keyed + cached by child run id. Called
     * when a child row is expanded. Idempotent: a cached `loaded`/`loading`
     * entry is a no-op, so re-expanding never refetches (a terminal run's
     * transcript is fixed); an `error` entry is retried on the next expand.
     *
     * A 404/any error stores `{status:'error'}` (rendered as status-only in the
     * card) rather than throwing — a pruned parent/child MUST NOT crash the card.
     */
    loadChildTranscript: async (childId: string): Promise<void> => {
      const existing = get().childDetailsById[childId]
      if (existing && existing.status !== 'error') return
      set(state => ({
        childDetailsById: {
          ...state.childDetailsById,
          [childId]: { status: 'loading' },
        },
      }))
      try {
        const detail = await ApiClient.SubAgentRuns.get({ child_id: childId })
        const activity: AgentActivityEntry[] = (detail.activity ?? []).filter(
          (e): e is AgentActivityEntry => e.type === 'agent_activity',
        )
        set(state => ({
          childDetailsById: {
            ...state.childDetailsById,
            [childId]: {
              status: 'loaded',
              activity,
              runStatus: detail.status,
            },
          },
        }))
      } catch (error) {
        console.error('Sub-agent transcript load failed:', childId, error)
        set(state => ({
          childDetailsById: {
            ...state.childDetailsById,
            [childId]: { status: 'error' },
          },
        }))
      }
    },
  }),
})

/** Augment ChatExtensionStores with SubAgentActivityStore
 *  (typed `Stores.Chat.SubAgentActivityStore`). */
declare module '../../types' {
  interface ChatExtensionStores {
    SubAgentActivityStore: ReturnType<typeof createSubAgentActivityStore>
  }
}
