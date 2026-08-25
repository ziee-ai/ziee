import { ChevronDown, Network } from 'lucide-react'
import { useState } from 'react'
import { Button, Card, Spin, Text } from '@ziee/kit'
import { cn } from '@/lib/utils'
import { ToolStatusIcon, ToolStatusInline } from '@/modules/chat/core/ToolStatusIcon'
import { AgentActivityTimeline } from '@/modules/workflow/components/run/AgentActivityTimeline'
import type { ChildTranscriptState } from '@/modules/chat/extensions/sub-agent-activity/SubAgentActivity.store'
import {
  subAgentChildToolStatus,
  subAgentRollupStatus,
  type SubAgentActivityVM,
  type SubAgentChildVM,
} from './agentActivity'

/**
 * ITEM-4 / ITEM-9 — a compact **delegated sub-agents** activity card in the chat
 * timeline. When the agent fans out to parallel sub-agents (a `delegate` call),
 * this surfaces the N children with a per-child running → done/failed status so
 * the user sees work happening beside the live chat. The header rollup mirrors
 * the whole fan-out's status. (The `subAgentActivity` frame carries only
 * `{ run_id, children }` — no merged-summary field — so the card shows the child
 * list, not a summary block.)
 *
 * ITEM-9 drill-in: each child row is LAZILY EXPANDABLE. Clicking a row fetches
 * that child's durable agent-loop transcript (via the injected `onExpandChild`
 * store action) and renders it inline with the shared workflow
 * `AgentActivityTimeline` — no separate drawer, mirroring how
 * `BackgroundRunCard.toggleResult` lazily loads on expand. The live status badge
 * stays SSE-driven; the drill-in only adds the after-the-fact transcript, and a
 * pruned child (transcript 404) degrades to status-only without crashing.
 *
 * Presentational + pure over its props: it takes the already-adapted activity VM
 * plus the transcript state map + expand handler the `message_footer` slot wires
 * to the pane store (the gallery renders it prop-only). Child rows reuse the
 * shared `ToolStatusIcon` so their status vocabulary can never drift from the
 * tool-call cards.
 */

export interface SubAgentActivityCardProps {
  activity: SubAgentActivityVM
  /** Per-child durable transcript state (ITEM-9), keyed by child run id. The
   *  footer supplies the pane store's `childDetailsById`; absent in the gallery. */
  childDetails?: Record<string, ChildTranscriptState>
  /** Kick the lazy transcript fetch for a child when its row is first expanded.
   *  Absent in the gallery (rows still toggle, showing a graceful empty state). */
  onExpandChild?: (childId: string) => void
  className?: string
  'data-testid'?: string
}

export function SubAgentActivityCard({
  activity,
  childDetails = {},
  onExpandChild,
  className,
  'data-testid': testId = 'agent-subagents-card',
}: SubAgentActivityCardProps) {
  const { children } = activity
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set())

  if (children.length === 0) return null
  const rollup = subAgentRollupStatus(children)

  const toggle = (childId: string): void => {
    setExpanded(prev => {
      const next = new Set(prev)
      if (next.has(childId)) {
        next.delete(childId)
      } else {
        next.add(childId)
        // Lazy-load the transcript only when the row is first opened; the store
        // caches it, so re-expanding a terminal child never refetches.
        onExpandChild?.(childId)
      }
      return next
    })
  }

  return (
    <Card
      size="sm"
      className={cn('mb-2', className)}
      data-testid={testId}
      aria-label="Delegated sub-agents"
    >
      <div className="flex items-center gap-2">
        <Network aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        <Text strong className="truncate">
          Delegated sub-agents
        </Text>
        <Text type="secondary" className="whitespace-nowrap text-xs">
          ({children.length})
        </Text>
        <ToolStatusInline status={rollup} className="ms-auto text-xs" />
      </div>

      <ul className="mt-2 flex flex-col gap-1.5" aria-label="Sub-agent runs">
        {children.map((child, index) => {
          const open = expanded.has(child.id)
          const panelId = `${testId}-child-panel-${index}`
          return (
            <li
              key={child.id}
              className="flex flex-col gap-1"
              data-testid={`${testId}-child-${index}`}
              data-status={child.status}
            >
              <Button
                variant="ghost"
                className="h-auto w-full justify-start gap-2 px-2 py-1"
                aria-expanded={open}
                aria-controls={panelId}
                data-testid={`${testId}-child-toggle-${index}`}
                onClick={() => toggle(child.id)}
              >
                <ToolStatusIcon status={subAgentChildToolStatus(child.status)} />
                <Text ellipsis className="min-w-0 flex-1 text-start text-sm">
                  {child.label}
                </Text>
                <ChevronDown
                  aria-hidden
                  className={cn(
                    'size-4 shrink-0 text-muted-foreground transition-transform',
                    open && 'rotate-180',
                  )}
                />
              </Button>
              {open && (
                <div
                  id={panelId}
                  className="ps-6"
                  data-testid={panelId}
                >
                  <ChildTranscript
                    child={child}
                    detail={childDetails[child.id]}
                    canFetch={!!onExpandChild}
                  />
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </Card>
  )
}

/**
 * The inline transcript body for one expanded child. Loading → spinner; error /
 * pruned (404) → a graceful status-only line (never a crash); loaded with rows →
 * the shared `AgentActivityTimeline`; loaded-but-empty → a "no transcript" note.
 * An undefined detail with a fetch handler present means the request was just
 * kicked (spinner); without a handler (gallery) it is a graceful empty state.
 */
function ChildTranscript({
  child,
  detail,
  canFetch,
}: {
  child: SubAgentChildVM
  detail: ChildTranscriptState | undefined
  canFetch: boolean
}) {
  const testIdBase = `agent-subagents-transcript-${child.id}`

  if (!detail) {
    if (canFetch) {
      return (
        <div className="flex justify-center py-2">
          <Spin label="Loading transcript" />
        </div>
      )
    }
    return (
      <Text type="secondary" className="text-xs" data-testid={`${testIdBase}-empty`}>
        No transcript recorded for this sub-agent.
      </Text>
    )
  }

  if (detail.status === 'loading') {
    return (
      <div className="flex justify-center py-2" data-testid={`${testIdBase}-loading`}>
        <Spin label="Loading transcript" />
      </div>
    )
  }

  if (detail.status === 'error') {
    return (
      <Text type="secondary" className="text-xs" data-testid={`${testIdBase}-error`}>
        This sub-agent's transcript is no longer available.
      </Text>
    )
  }

  if (detail.activity.length === 0) {
    return (
      <Text type="secondary" className="text-xs" data-testid={`${testIdBase}-empty`}>
        No transcript recorded for this sub-agent.
      </Text>
    )
  }

  return <AgentActivityTimeline stepId={child.id} entries={detail.activity} />
}
