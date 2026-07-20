/**
 * Pure view-models + render helpers for the two agent-orchestration chat
 * surfaces:
 *   - the agent's live **task list** (Group G / ITEM-36), and
 *   - **delegated sub-agent activity** (Group A / ITEM-4).
 *
 * PURE module — no `@/` (or React/lucide) runtime imports, so it stays
 * unit-testable under `node --test` (mirrors `chat/core/tool-status.ts`). The
 * presentational components import the types + helpers here; the icon/color
 * vocabulary lives in the components (and, for sub-agent children, is reused
 * from the shared `ToolStatusIcon`).
 *
 * ## The shapes mirror agent-core Rust — which is NOT YET in the FE api-client
 * `agent-core/src/types.rs` defines `TaskItem` / `TaskStatus` (`pending |
 * in_progress | completed`) and `SubagentSummary`, and emits
 * `AgentEvent::TaskListChanged { run_id, items }`. As of this tranche the chat
 * host DROPS that event (`server/src/modules/chat/agent_host/event_sink.rs`
 * → `AgentEvent::TaskListChanged { .. } => {}`) and there is NO
 * `SSEChatStreamEventVariants` variant for it, so neither the frame nor
 * `TaskItem` reaches the generated `api-client/types.ts`. These VMs are the
 * FE-local mirror; `taskItemsFromFrame` / `subAgentActivityFromChildren` are the
 * single adapter seam the SSE handler plugs into once the frame lands (see the
 * plumbing FLAG in the tranche report). They deliberately avoid `as any` on a
 * yet-untyped frame.
 */

/** A task-list item's lifecycle status (mirrors Rust `TaskStatus`, snake_case
 *  on the wire so `status: "in_progress"` deserializes directly). DISTINCT from
 *  the generated `TaskStatus` (`running | completed | failed`), which is the
 *  background-run status — not this. */
export type TaskItemStatus = 'pending' | 'in_progress' | 'completed'

/** One agent task-list item (mirrors Rust `TaskItem`). `content` is the
 *  imperative form ("Run tests"); `active_form` the present-continuous form
 *  ("Running tests") rendered while the item is `in_progress`. */
export interface TaskItemVM {
  id: string
  content: string
  active_form: string
  status: TaskItemStatus
}

/**
 * Claude-Code's render rule: an `in_progress` item is shown by its
 * `active_form` ("Running tests"); every other item by its `content` ("Run
 * tests"). Falls back across the two fields so a surface never renders a blank
 * row when the model omitted one form.
 */
export function taskItemLabel(item: TaskItemVM): string {
  if (item.status === 'in_progress') return item.active_form || item.content
  return item.content || item.active_form
}

export interface TaskListCounts {
  completed: number
  inProgress: number
  pending: number
  total: number
}

/** Roll up per-status counts for the checklist header ("2 / 5"). */
export function taskListCounts(items: readonly TaskItemVM[]): TaskListCounts {
  let completed = 0
  let inProgress = 0
  let pending = 0
  for (const it of items) {
    if (it.status === 'completed') completed++
    else if (it.status === 'in_progress') inProgress++
    else pending++
  }
  return { completed, inProgress, pending, total: items.length }
}

/** Adapter seam: the future `taskListChanged` SSE frame carries `{ run_id,
 *  items }`. This is where the (still-untyped) frame's `items` become VMs — a
 *  defensive passthrough today so the SSE handler is a one-liner once the frame
 *  is generated into the api-client. */
export function taskItemsFromFrame(frame: { items?: TaskItemVM[] | null }): TaskItemVM[] {
  return frame.items ?? []
}

// ---------------------------------------------------------------------------
// Sub-agent / delegate activity (Group A / ITEM-4)
// ---------------------------------------------------------------------------

/** A delegated child's status. `completed`/`failed` are terminal. */
export type SubAgentChildStatus = 'running' | 'completed' | 'failed'

/** One delegated sub-agent in a fan-out (a `delegate` call spawns N of these).
 *  `label` is the friendly per-child descriptor (its objective / role). */
export interface SubAgentChildVM {
  id: string
  label: string
  status: SubAgentChildStatus
}

/** A `delegate` fan-out's live activity: the children + (when finished) the
 *  merged `SubagentSummary` text the parent reads back. */
export interface SubAgentActivityVM {
  children: SubAgentChildVM[]
  /** The merged child summary (Rust `SubagentSummary.summary`), present once
   *  the fan-out completes; the parent never sees child transcripts (P9). */
  summary?: string | null
}

/**
 * Normalize a child status to the shared tool-status vocabulary
 * (`running | success | failed`) so a child row renders through the SAME
 * `ToolStatusIcon` as every tool-call card — one status-icon source, no drift.
 */
export function subAgentChildToolStatus(
  status: SubAgentChildStatus,
): 'running' | 'success' | 'failed' {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'failed'
  return 'running'
}

/**
 * Roll up the whole fan-out's status for the card header icon: `failed` if ANY
 * child failed, else `running` while ANY child is still running, else
 * `success`. An empty set reads as `running` (spawn in flight).
 */
export function subAgentRollupStatus(
  children: readonly SubAgentChildVM[],
): 'running' | 'success' | 'failed' {
  if (children.some(c => c.status === 'failed')) return 'failed'
  if (children.length === 0 || children.some(c => c.status === 'running')) return 'running'
  return 'success'
}

/** Adapter seam for the future sub-agent-activity SSE frame / content-block
 *  (DEC-65). Present so the eventual SSE handler builds a VM without `as any`. */
export function subAgentActivityFromChildren(
  children: SubAgentChildVM[],
  summary?: string | null,
): SubAgentActivityVM {
  return { children, summary: summary ?? null }
}
