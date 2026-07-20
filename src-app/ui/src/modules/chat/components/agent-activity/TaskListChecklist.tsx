import { Circle, CheckCircle2, Loader2, ListTodo, type LucideIcon } from 'lucide-react'
import { Card, Text } from '@ziee/kit'
import { cn } from '@/lib/utils'
import {
  taskItemLabel,
  taskListCounts,
  type TaskItemStatus,
  type TaskItemVM,
} from './agentActivity'

/**
 * ITEM-36 — the agent's live, self-managed **task list** rendered inline in the
 * assistant turn as an evolving checklist (Claude-Code `Task`-tools style). It
 * is a *followable stream*: the list re-renders in place as
 * `AgentEvent::TaskListChanged` frames arrive, each carrying the full current
 * list. The `in_progress` item is emphasised and shown by its present-continuous
 * `active_form` ("Running tests"); every other item shows its imperative
 * `content` ("Run tests") — the CC dual-form rule (see `taskItemLabel`).
 *
 * Presentational + pure: it takes the already-adapted `items` VM and holds no
 * store/SSE wiring (the live data source — a `taskListChanged` SSE frame — is
 * not yet in the generated api-client; see the tranche's plumbing FLAG). Status
 * icons use design-system tokens only.
 */

/** Per-status icon + color + spin, in the semantic-token vocabulary. A
 *  `pending` item is a hollow neutral circle (NOT the tool-card "running"
 *  spinner — a not-started task must not read as in-flight). */
const TASK_ITEM_STATUS: Record<
  TaskItemStatus,
  { icon: LucideIcon; color: string; spin: boolean; label: string }
> = {
  pending: { icon: Circle, color: 'text-muted-foreground', spin: false, label: 'To do' },
  in_progress: { icon: Loader2, color: 'text-primary', spin: true, label: 'In progress' },
  completed: { icon: CheckCircle2, color: 'text-success', spin: false, label: 'Completed' },
}

export interface TaskListChecklistProps {
  items: TaskItemVM[]
  className?: string
  'data-testid'?: string
}

export function TaskListChecklist({
  items,
  className,
  'data-testid': testId = 'agent-task-list',
}: TaskListChecklistProps) {
  if (items.length === 0) return null
  const counts = taskListCounts(items)

  return (
    <Card
      size="sm"
      className={cn('mb-2', className)}
      data-testid={testId}
      aria-label="Agent task list"
    >
      <div className="flex items-center gap-2">
        <ListTodo aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        <Text strong className="truncate">
          Task list
        </Text>
        <Text
          type="secondary"
          className="ms-auto whitespace-nowrap text-xs"
          data-testid={`${testId}-count`}
        >
          {counts.completed}/{counts.total}
        </Text>
      </div>

      <ol className="mt-2 flex flex-col gap-1.5">
        {items.map((item, index) => {
          const d = TASK_ITEM_STATUS[item.status]
          const Icon = d.icon
          const active = item.status === 'in_progress'
          const done = item.status === 'completed'
          return (
            <li
              key={item.id}
              className="flex items-start gap-2"
              aria-current={active ? 'step' : undefined}
              data-testid={`${testId}-item-${index}`}
              data-status={item.status}
            >
              <Icon
                aria-hidden
                className={cn('mt-0.5 size-4 shrink-0', d.color, d.spin && 'animate-spin')}
              />
              <span className="sr-only">{d.label}: </span>
              <Text
                className={cn(
                  'text-sm',
                  done && 'text-muted-foreground line-through',
                  active && 'font-medium text-foreground',
                  !done && !active && 'text-foreground',
                )}
              >
                {taskItemLabel(item)}
              </Text>
            </li>
          )
        })}
      </ol>
    </Card>
  )
}
