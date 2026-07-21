import {
  ChevronDown,
  ChevronRight,
  MessagesSquare,
  MoreHorizontal,
  Pencil,
  Play,
  Repeat,
  Trash2,
} from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'

import type { ScheduledTask, ScheduledTaskRun } from '@/api-client/types'
import { ListPagination } from '@/components/common/ListPagination'
import {
  Badge,
  Button,
  Card,
  Confirm,
  Dropdown,
  Flex,
  message,
  Spin,
  Switch,
  Tag,
  Text,
  Title,
  Tooltip,
} from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { cn } from '@/lib/utils'

import {
  changeBadge,
  followupActions,
  RUNS_PAGE_SIZE,
  runPreviewLine,
  seriesChoices,
} from './runTimeline'
import { humanizeCron } from './scheduleCron'
import { skippedToolsNote } from './skippedToolsNote'

/** Store mutations don't surface their own errors (no error state), so the UI
 *  layer toasts a rejected action rather than swallowing it. */
const notifyError = (e: unknown, fallback: string) =>
  message.error(e instanceof Error ? e.message : fallback)

function targetSummary(t: ScheduledTask): string {
  return t.target_kind === 'workflow' ? 'Workflow' : 'Prompt'
}

function scheduleSummary(t: ScheduledTask): string {
  if (t.schedule_kind === 'self_paced') {
    // A self-paced "loop" (/loop) carries neither run_at nor cron — after each
    // firing the model proposes its own next cadence, so there is no fixed cron
    // "next run" to show. Describe the loop semantics instead.
    return 'Self-paced loop — runs, then decides its own next check'
  }
  if (t.schedule_kind === 'once') {
    return t.run_at ? `Once at ${new Date(t.run_at).toLocaleString()}` : 'Once'
  }
  return `${humanizeCron(t.cron_expr ?? '')} (${t.timezone})`
}

/**
 * The status line under the schedule summary. A self-paced loop phrases its
 * upcoming firing as a model-chosen "next check" (and reads "finished" once it
 * self-stops, `paused_reason === 'completed'`); a scheduled task keeps the cron
 * "Next: …" wording. `last_status` is appended when known.
 */
function nextRunLine(t: ScheduledTask): string {
  if (t.schedule_kind === 'self_paced') {
    if (t.paused_reason === 'completed') return 'Loop finished'
    const base = t.next_run_at
      ? `Next check: ${new Date(t.next_run_at).toLocaleString()}`
      : 'Deciding its next check'
    return t.last_status ? `${base} · Last: ${t.last_status}` : base
  }
  const base = t.next_run_at
    ? `Next: ${new Date(t.next_run_at).toLocaleString()}`
    : 'No upcoming run'
  return t.last_status ? `${base} · Last: ${t.last_status}` : base
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
          navigate(`/chat/${a.threadConversationId}`)
      },
    })
  }
  items.push({
    key: 'fork',
    label: a.forkLabel,
    onClick: async () => {
      try {
        const conversationId = await Stores.ScheduledTasks.continueRun(run.id)
        if (conversationId) navigate(`/chat/${conversationId}`)
      } catch (e) {
        notifyError(e, 'Failed to open the conversation')
      }
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
          <Dropdown
            items={items}
            data-testid={`run-actions-menu-content-${run.id}`}
          >
            {/* role=button (not a raw <button>) so the kit lint + Base UI trigger
                are satisfied; the Dropdown trigger handles keyboard activation.
                testid lives on the TRIGGER (the Dropdown prop would tag the menu
                content, which only mounts when open). */}
            <div
              role="button"
              tabIndex={0}
              aria-label="Run actions"
              data-testid={`run-actions-menu-${run.id}`}
              className="hover:bg-muted inline-flex h-8 w-8 items-center justify-center rounded-md"
            >
              <MoreHorizontal size={16} />
            </div>
          </Dropdown>
        </div>
      </Flex>

      {open && (
        <div
          id={detailId}
          className="mt-1 border-t pt-1"
          data-testid={detailId}
        >
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
    try {
      const conversationId = await Stores.ScheduledTasks.continueSeries(
        task.id,
        limit,
      )
      if (conversationId) navigate(`/chat/${conversationId}`)
    } catch (e) {
      notifyError(e, 'Failed to start the discussion')
    }
  }
  const items = seriesChoices(loadedCount).map(c => ({
    key: String(c.value),
    label: c.label,
    onClick: () => void start(c.value),
  }))
  return (
    <Dropdown items={items} data-testid={`series-chooser-menu-${task.id}`}>
      <div
        role="button"
        tabIndex={0}
        aria-label="Discuss recent runs"
        data-testid={`series-chooser-${task.id}`}
        className="text-muted-foreground hover:text-foreground inline-flex cursor-pointer items-center gap-1 text-xs"
      >
        Discuss recent runs
        <ChevronDown size={14} />
      </div>
    </Dropdown>
  )
}

/**
 * ITEM-52: the single scheduled-task card, extracted from the list page and
 * mirroring KnowledgeBaseCard/ProjectCard (Card `title`/`extra`, `!font-normal
 * !text-sm` list-item title, hover-revealed `outline size="icon"` + Tooltip
 * actions). The enable/disable Switch stays ALWAYS-visible — it conveys STATE,
 * not an action (DEC-25). Single-column expandable-runs layout retained (DEC-24).
 */
export function ScheduledTaskCard({ task }: { task: ScheduledTask }) {
  const navigate = useNavigate()
  const [expanded, setExpanded] = useState(false)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const runs = Stores.ScheduledTasks.runsByTask[task.id]
  const meta = Stores.ScheduledTasks.runsMetaByTask[task.id]
  const total = meta?.total ?? runs?.length ?? 0
  const page = meta?.page ?? 1
  const perPage = meta?.perPage ?? RUNS_PAGE_SIZE
  const threadActions = followupActions(task)
  // A self-paced "loop" (/loop) vs a cron/once scheduled task — drives the Loop
  // badge, the "next check" wording, and the in-context conversation link.
  const isSelfPaced = task.schedule_kind === 'self_paced'

  const toggleRuns = () => {
    const next = !expanded
    setExpanded(next)
    if (next && !runs) void Stores.ScheduledTasks.loadRuns(task.id, 1)
  }

  return (
    <Card
      data-testid={`task-card-${task.id}`}
      className="group"
      title={
        // The name + its enable Switch form one unit at the left — the Switch is
        // STATE and belongs beside the task it governs, not stranded at the far
        // right of the header. The kind Tag + status Badge live in the card body
        // (they'd clip in the truncating CardTitle) — mirrors KnowledgeBaseCard.
        <div className="flex min-w-0 items-center gap-2">
          <Title
            level={5}
            data-testid={`task-name-${task.id}`}
            className="!m-0 min-w-0 truncate !text-sm !font-normal [overflow-wrap:anywhere]"
          >
            {task.name}
          </Title>
          <Switch
            data-standalone-control
            data-testid={`task-enabled-${task.id}`}
            className="shrink-0"
            aria-label={task.enabled ? 'Disable task' : 'Enable task'}
            checked={task.enabled}
            onCheckedChange={async v => {
              try {
                await Stores.ScheduledTasks.setEnabled(task.id, v)
              } catch (e) {
                notifyError(e, 'Failed to update the task')
              }
            }}
          />
        </div>
      }
      extra={
        <>
          {/* Actions (hover/focus-revealed, always-on for touch): mirror ProjectCard. */}
          <Flex
            data-testid={`task-actions-${task.id}`}
            className={cn(
              'items-center gap-1 transition-opacity',
              deleteOpen
                ? 'opacity-100'
                : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100',
            )}
          >
            {task.target_kind === 'prompt' && (
              <Tooltip content="Open thread">
                <Button
                  data-testid={`task-open-thread-${task.id}`}
                  variant="outline"
                  size="icon"
                  icon={<MessagesSquare />}
                  aria-label="Open thread"
                  disabled={threadActions.openThread === 'disabled'}
                  onClick={() => {
                    if (threadActions.threadConversationId)
                      navigate(
                        `/chat/${threadActions.threadConversationId}`,
                      )
                  }}
                />
              </Tooltip>
            )}
            <Tooltip content="Run now">
              <Button
                data-testid={`task-run-now-${task.id}`}
                variant="outline"
                size="icon"
                icon={<Play />}
                aria-label="Run now"
                onClick={async () => {
                  try {
                    await Stores.ScheduledTasks.runNow(task.id)
                    message.info(
                      'Running now — result will land in your notifications',
                    )
                  } catch (e) {
                    notifyError(e, 'Failed to run the task')
                  }
                }}
              />
            </Tooltip>
            <Tooltip content="Edit">
              <Button
                data-testid={`task-edit-${task.id}`}
                variant="outline"
                size="icon"
                icon={<Pencil />}
                aria-label="Edit"
                onClick={() => Stores.SchedulerDrawer.openEdit(task)}
              />
            </Tooltip>
            <Tooltip content="Delete">
              <Button
                data-testid={`task-delete-${task.id}`}
                variant="outline"
                size="icon"
                icon={<Trash2 />}
                aria-label="Delete"
                loading={deleting}
                onClick={() => setDeleteOpen(true)}
              />
            </Tooltip>
            <Confirm
              data-testid={`task-delete-confirm-${task.id}`}
              open={deleteOpen}
              onOpenChange={setDeleteOpen}
              title="Delete scheduled task"
              description={`Delete "${task.name}"? Its run history is removed. This cannot be undone.`}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
              onConfirm={async () => {
                setDeleting(true)
                try {
                  await Stores.ScheduledTasks.deleteTask(task.id)
                  // success → the card unmounts (task removed from the list).
                } catch (e) {
                  notifyError(e, 'Failed to delete the task')
                  setDeleting(false)
                }
              }}
            />
          </Flex>
        </>
      }
    >
      <Flex className="mb-1 flex-wrap items-center gap-2">
        {isSelfPaced && (
          <Tag
            tone="info"
            icon={<Repeat />}
            data-testid={`task-loop-${task.id}`}
          >
            Loop
          </Tag>
        )}
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
      <Text className="text-muted-foreground block text-sm">
        {scheduleSummary(task)}
      </Text>
      {task.completion_condition && (
        <Text
          className="text-muted-foreground block text-xs [overflow-wrap:anywhere]"
          data-testid={`task-stop-condition-${task.id}`}
        >
          Stops when: {task.completion_condition}
        </Text>
      )}
      <Text className="text-muted-foreground block text-xs">
        {nextRunLine(task)}
      </Text>
      {isSelfPaced && task.bound_conversation_id && (
        // The loop runs INSIDE a conversation — surface that context as a link
        // to it (the JTBD: "know where this loop lives"). Uses SPA navigation to
        // the real chat route `/chat/:id`.
        <Button
          variant="link"
          icon={<MessagesSquare />}
          data-testid={`task-bound-conversation-${task.id}`}
          className="mt-0.5 h-auto justify-start gap-1 p-0 text-xs"
          onClick={() => navigate(`/chat/${task.bound_conversation_id}`)}
        >
          Open its conversation
        </Button>
      )}
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
    </Card>
  )
}
