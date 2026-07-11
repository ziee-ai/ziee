import {
  ChevronDown,
  ChevronRight,
  MessagesSquare,
  MoreHorizontal,
  Pencil,
  Play,
  Trash2,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'

import type { ScheduledTask, ScheduledTaskRun } from '@/api-client/types'
import { ListPagination } from '@/components/common/ListPagination'
import {
  Badge,
  Button,
  Card,
  Dropdown,
  Empty,
  ErrorState,
  Flex,
  Spin,
  Switch,
  Tag,
  Text,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

import {
  changeBadge,
  followupActions,
  RUNS_PAGE_SIZE,
  runPreviewLine,
  seriesChoices,
} from '../components/runTimeline'
import { humanizeCron } from '../components/scheduleCron'
import { skippedToolsNote } from '../components/skippedToolsNote'

function targetSummary(t: ScheduledTask): string {
  return t.target_kind === 'workflow' ? 'Workflow' : 'Prompt'
}

function scheduleSummary(t: ScheduledTask): string {
  if (t.schedule_kind === 'once') {
    return t.run_at ? `Once at ${new Date(t.run_at).toLocaleString()}` : 'Once'
  }
  return `${humanizeCron(t.cron_expr ?? '')} (${t.timezone})`
}

type NavigateFn = (to: string) => void

interface RunAction {
  key: string
  label: string
  onClick: () => void
  disabled?: boolean
}

/**
 * ITEM-45 (DEC-21): per-run follow-up actions — "Open thread" (resume the bound
 * conversation, prompt tasks) is primary; the fork ("New side chat" / "Continue in
 * chat") is always present. Shared by the inline buttons + the mobile overflow menu.
 */
function runActionItems(
  task: ScheduledTask,
  run: ScheduledTaskRun,
  navigate: NavigateFn,
): RunAction[] {
  const a = followupActions(task)
  const items: RunAction[] = []
  if (a.openThread !== 'none') {
    items.push({
      key: 'open-thread',
      label: 'Open thread',
      disabled: a.openThread === 'disabled',
      onClick: () => {
        if (a.threadConversationId)
          navigate(`/conversations/${a.threadConversationId}`)
      },
    })
  }
  items.push({
    key: 'fork',
    label: a.forkLabel,
    onClick: async () => {
      const conversationId = await Stores.ScheduledTasks.continueRun(run.id)
      if (conversationId) navigate(`/conversations/${conversationId}`)
    },
  })
  return items
}

/** ITEM-44: one run in the timeline — what-changed badge + preview, click to expand. */
function RunRow({ task, run }: { task: ScheduledTask; run: ScheduledTaskRun }) {
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const badge = changeBadge(run)
  const preview = runPreviewLine(run)
  const skip = skippedToolsNote(run.skipped_tools)
  const items = runActionItems(task, run, navigate)
  const detailId = `run-detail-${run.id}`

  return (
    <div data-testid={`run-row-${run.id}`} className="rounded-md border p-2">
      <Flex className="items-start justify-between gap-2">
        <Button
          variant="ghost"
          className="h-auto min-w-0 flex-1 justify-start gap-1 p-0 text-start font-normal hover:bg-transparent"
          data-testid={`run-expand-${run.id}`}
          aria-expanded={open}
          aria-controls={detailId}
          onClick={() => setOpen(v => !v)}
        >
          <span className="text-muted-foreground mt-0.5 shrink-0">
            {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </span>
          <span className="min-w-0 flex-1">
            <Flex className="items-center gap-2">
              <Text className="text-muted-foreground whitespace-nowrap text-xs">
                {new Date(run.fired_at).toLocaleString()}
              </Text>
              {badge && (
                <Badge tone={badge.tone} data-testid={`run-badge-${run.id}`}>
                  {badge.label}
                </Badge>
              )}
            </Flex>
            {preview && (
              <Text
                className={`text-sm ${open ? '' : 'truncate'}`}
                data-testid={`run-preview-${run.id}`}
              >
                {preview}
              </Text>
            )}
          </span>
        </Button>

        {/* Actions: inline on ≥sm, overflow menu on mobile (ITEM-48). */}
        <div className="hidden items-center gap-1 sm:flex">
          {items.map(it => (
            <Button
              key={it.key}
              variant="ghost"
              className="h-auto px-1 py-0 text-xs"
              disabled={it.disabled}
              data-testid={`run-action-${it.key}-${run.id}`}
              onClick={it.onClick}
            >
              {it.label}
            </Button>
          ))}
        </div>
        <div className="sm:hidden">
          <Dropdown items={items} data-testid={`run-actions-menu-${run.id}`}>
            {/* role=button (not a raw <button>) so the kit lint + Base UI trigger
                are satisfied; the Dropdown trigger handles keyboard activation. */}
            <div
              role="button"
              tabIndex={0}
              aria-label="Run actions"
              className="hover:bg-muted inline-flex h-8 w-8 items-center justify-center rounded-md"
            >
              <MoreHorizontal size={16} />
            </div>
          </Dropdown>
        </div>
      </Flex>

      {open && (
        <div id={detailId} className="mt-1 border-t pt-1" data-testid={detailId}>
          {run.status === 'failed' && run.error_message && (
            <Text className="text-destructive text-xs">
              {run.error_class ? `${run.error_class}: ` : ''}
              {run.error_message}
            </Text>
          )}
          {skip && (
            <Text
              className="text-muted-foreground text-xs"
              data-testid={`run-skipped-${run.id}`}
            >
              {skip}
            </Text>
          )}
          {!preview && run.status !== 'failed' && (
            <Text className="text-muted-foreground text-xs">
              No result text captured.
            </Text>
          )}
        </div>
      )}
    </div>
  )
}

/** ITEM-47 (DEC-22): the "Discuss recent runs" action menu {5, 10, all-loaded}. */
function SeriesChooser({
  task,
  loadedCount,
}: {
  task: ScheduledTask
  loadedCount: number
}) {
  const navigate = useNavigate()
  const start = async (limit: number) => {
    const conversationId = await Stores.ScheduledTasks.continueSeries(task.id, limit)
    if (conversationId) navigate(`/conversations/${conversationId}`)
  }
  const items = seriesChoices(loadedCount).map(c => ({
    key: String(c.value),
    label: c.label,
    onClick: () => void start(c.value),
  }))
  return (
    <Dropdown items={items} data-testid={`series-chooser-${task.id}`}>
      <div
        role="button"
        tabIndex={0}
        aria-label="Discuss recent runs"
        className="text-muted-foreground hover:text-foreground inline-flex cursor-pointer items-center gap-1 text-xs"
      >
        Discuss recent runs
        <ChevronDown size={14} />
      </div>
    </Dropdown>
  )
}

function TaskRow({ task }: { task: ScheduledTask }) {
  const navigate = useNavigate()
  const [expanded, setExpanded] = useState(false)
  const runs = Stores.ScheduledTasks.runsByTask[task.id]
  const meta = Stores.ScheduledTasks.runsMetaByTask[task.id]
  const total = meta?.total ?? runs?.length ?? 0
  const page = meta?.page ?? 1
  const perPage = meta?.perPage ?? RUNS_PAGE_SIZE
  const threadActions = followupActions(task)

  const toggleRuns = () => {
    const next = !expanded
    setExpanded(next)
    if (next && !runs) void Stores.ScheduledTasks.loadRuns(task.id, 1)
  }

  return (
    <Card data-testid={`task-card-${task.id}`}>
      <Flex className="items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <Flex className="items-center gap-2">
            <Text className="truncate font-medium">{task.name}</Text>
            <Tag data-testid={`task-kind-${task.id}`}>{targetSummary(task)}</Tag>
            {task.paused_reason === 'completed' ? (
              <Badge tone="success" data-testid={`task-completed-${task.id}`}>
                Completed
              </Badge>
            ) : (
              task.paused_reason && (
                <Badge tone="error" data-testid={`task-paused-${task.id}`}>
                  Paused: {task.paused_reason}
                </Badge>
              )
            )}
          </Flex>
          <Text className="text-muted-foreground text-sm">
            {scheduleSummary(task)}
          </Text>
          <Text className="text-muted-foreground text-xs">
            {task.next_run_at
              ? `Next: ${new Date(task.next_run_at).toLocaleString()}`
              : 'No upcoming run'}
            {task.last_status ? ` · Last: ${task.last_status}` : ''}
          </Text>
          <Button
            data-testid={`task-runs-toggle-${task.id}`}
            variant="ghost"
            className="mt-1 px-0"
            onClick={toggleRuns}
          >
            {expanded ? 'Hide runs' : 'Show runs'}
          </Button>
          {expanded && (
            <div className="mt-1 border-t pt-2">
              {!runs ? (
                <Spin label="Loading runs" />
              ) : total === 0 ? (
                <Text className="text-muted-foreground text-xs">No runs yet</Text>
              ) : (
                <Flex className="flex-col gap-2">
                  <Flex className="items-center justify-between gap-2">
                    <Text
                      className="text-muted-foreground text-xs"
                      data-testid={`runs-count-${task.id}`}
                    >
                      Showing {runs.length} of {total}
                    </Text>
                    <SeriesChooser task={task} loadedCount={runs.length} />
                  </Flex>
                  {runs.map(r => (
                    <RunRow key={r.id} task={task} run={r} />
                  ))}
                  {total > perPage && (
                    <ListPagination
                      data-testid={`runs-pagination-${task.id}`}
                      current={page}
                      total={total}
                      pageSize={perPage}
                      itemNoun="run"
                      onChange={p =>
                        void Stores.ScheduledTasks.loadRuns(task.id, p, perPage)
                      }
                      onPageSizeChange={size =>
                        void Stores.ScheduledTasks.loadRuns(task.id, 1, size)
                      }
                    />
                  )}
                </Flex>
              )}
            </div>
          )}
        </div>
        <Flex className="items-center gap-1">
          {task.target_kind === 'prompt' && (
            <Button
              data-testid={`task-open-thread-${task.id}`}
              variant="ghost"
              aria-label="Open thread"
              disabled={threadActions.openThread === 'disabled'}
              title={
                threadActions.openThread === 'disabled'
                  ? 'Runs once, then you can open the thread'
                  : 'Open the conversation thread'
              }
              onClick={() => {
                if (threadActions.threadConversationId)
                  navigate(`/conversations/${threadActions.threadConversationId}`)
              }}
            >
              <MessagesSquare size={16} />
            </Button>
          )}
          <Switch
            data-standalone-control
            data-testid={`task-enabled-${task.id}`}
            aria-label={task.enabled ? 'Disable task' : 'Enable task'}
            checked={task.enabled}
            onCheckedChange={v => void Stores.ScheduledTasks.setEnabled(task.id, v)}
          />
          <Button
            data-testid={`task-run-now-${task.id}`}
            variant="ghost"
            aria-label="Run now"
            onClick={async () => {
              await Stores.ScheduledTasks.runNow(task.id)
              message.info('Running now — result will land in your notifications')
            }}
          >
            <Play size={16} />
          </Button>
          <Button
            data-testid={`task-edit-${task.id}`}
            variant="ghost"
            aria-label="Edit"
            onClick={() => Stores.SchedulerDrawer.openEdit(task)}
          >
            <Pencil size={16} />
          </Button>
          <Button
            data-testid={`task-delete-${task.id}`}
            variant="ghost"
            aria-label="Delete"
            onClick={() => void Stores.ScheduledTasks.deleteTask(task.id)}
          >
            <Trash2 size={16} />
          </Button>
        </Flex>
      </Flex>
    </Card>
  )
}

export function ScheduledTasksPage() {
  const { tasks, loading, error } = Stores.ScheduledTasks

  useEffect(() => {
    void Stores.ScheduledTasks.loadTasks()
  }, [])

  return (
    <SettingsPageContainer
      title="Scheduled Tasks"
      subtitle="Run workflows or prompts on a schedule while you're away."
      data-testid="scheduled-tasks-page"
    >
      <Flex className="mb-3 justify-end">
        <Button
          data-testid="scheduled-tasks-new"
          onClick={() => Stores.SchedulerDrawer.openCreate()}
        >
          New task
        </Button>
      </Flex>

      {loading && tasks.length === 0 ? (
        <Flex className="justify-center py-12">
          <Spin size="lg" label="Loading scheduled tasks" />
        </Flex>
      ) : error && tasks.length === 0 ? (
        <ErrorState
          variant="page"
          resource="scheduled tasks"
          details={error}
          onRetry={() => void Stores.ScheduledTasks.loadTasks()}
          data-testid="scheduled-tasks-error"
        />
      ) : tasks.length === 0 ? (
        <Empty
          description="No scheduled tasks yet. Create one to have ziee work while you're away."
          data-testid="scheduled-tasks-empty"
        />
      ) : (
        <Flex className="flex-col gap-2">
          {tasks.map(t => (
            <TaskRow key={t.id} task={t} />
          ))}
        </Flex>
      )}
    </SettingsPageContainer>
  )
}
