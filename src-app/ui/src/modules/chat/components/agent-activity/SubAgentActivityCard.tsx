import { Network } from 'lucide-react'
import { Card, Text } from '@ziee/kit'
import { cn } from '@/lib/utils'
import { ToolStatusIcon, ToolStatusInline } from '@/modules/chat/core/ToolStatusIcon'
import {
  subAgentChildToolStatus,
  subAgentRollupStatus,
  type SubAgentActivityVM,
} from './agentActivity'

/**
 * ITEM-4 — a compact **delegated sub-agents** activity card in the chat
 * timeline. When the agent fans out to parallel sub-agents (a `delegate` call),
 * this surfaces the N children with a per-child running → done/failed status so
 * the user sees work happening beside the live chat (the parent never sees child
 * transcripts — P9 / DEC-53). The header rollup mirrors the whole fan-out's
 * status. (The `subAgentActivity` frame carries only `{ run_id, children }` — no
 * merged-summary field — so the card shows the child list, not a summary block.)
 *
 * Presentational + pure: it takes the already-adapted activity VM. The live data
 * source (a sub-agent-activity SSE frame / content-block, DEC-65) is not yet in
 * the generated api-client — see the tranche's plumbing FLAG. Child rows reuse
 * the shared `ToolStatusIcon` so their status vocabulary can never drift from
 * the tool-call cards.
 */

export interface SubAgentActivityCardProps {
  activity: SubAgentActivityVM
  className?: string
  'data-testid'?: string
}

export function SubAgentActivityCard({
  activity,
  className,
  'data-testid': testId = 'agent-subagents-card',
}: SubAgentActivityCardProps) {
  const { children } = activity
  if (children.length === 0) return null
  const rollup = subAgentRollupStatus(children)

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
        <ToolStatusInline
          status={rollup}
          className="ms-auto text-xs"
        />
      </div>

      <ul className="mt-2 flex flex-col gap-1.5" aria-label="Sub-agent runs">
        {children.map((child, index) => (
          <li
            key={child.id}
            className="flex items-center gap-2"
            data-testid={`${testId}-child-${index}`}
            data-status={child.status}
          >
            <ToolStatusIcon status={subAgentChildToolStatus(child.status)} />
            <Text ellipsis className="min-w-0 flex-1 text-sm">
              {child.label}
            </Text>
          </li>
        ))}
      </ul>
    </Card>
  )
}
