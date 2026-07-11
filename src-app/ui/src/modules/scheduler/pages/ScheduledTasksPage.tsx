import { Pencil, Play, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'

import type { ScheduledTask } from '@/api-client/types'
import {
  Badge,
  Button,
  Card,
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

function TaskRow({ task }: { task: ScheduledTask }) {
  const [expanded, setExpanded] = useState(false)
  const runs = Stores.ScheduledTasks.runsByTask[task.id]

  const toggleRuns = () => {
    const next = !expanded
    setExpanded(next)
    if (next && !runs) void Stores.ScheduledTasks.loadRuns(task.id)
  }

  return (
    <Card data-testid={`task-card-${task.id}`}>
      <Flex className="items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <Flex className="items-center gap-2">
            <Text className="truncate font-medium">{task.name}</Text>
            <Tag data-testid={`task-kind-${task.id}`}>
              {targetSummary(task)}
            </Tag>
            {task.paused_reason === 'completed' ? (
              // A spent `once` task is DONE, not failed — a distinct neutral badge
              // (never the error-toned "Paused" surface).
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
              ) : runs.length === 0 ? (
                <Text className="text-muted-foreground text-xs">
                  No runs yet
                </Text>
              ) : (
                <Flex className="flex-col gap-1">
                  {runs.map(r => (
                    <Flex key={r.id} className="items-center gap-2">
                      <Text className="text-muted-foreground text-xs">
                        {new Date(r.fired_at).toLocaleString()} — {r.status}
                        {r.error_class ? ` (${r.error_class})` : ''}
                      </Text>
                      {skippedToolsNote(r.skipped_tools) && (
                        <Text
                          className="text-muted-foreground text-xs"
                          data-testid={`run-skipped-${r.id}`}
                        >
                          {skippedToolsNote(r.skipped_tools)}
                        </Text>
                      )}
                      <Button
                        data-testid={`run-continue-${r.id}`}
                        variant="ghost"
                        className="h-auto px-1 py-0 text-xs"
                        onClick={async () => {
                          const conversationId =
                            await Stores.ScheduledTasks.continueRun(r.id)
                          if (conversationId) {
                            window.location.href = `/conversations/${conversationId}`
                          }
                        }}
                      >
                        Continue in chat
                      </Button>
                    </Flex>
                  ))}
                </Flex>
              )}
            </div>
          )}
        </div>
        <Flex className="items-center gap-1">
          <Switch
            data-standalone-control
            data-testid={`task-enabled-${task.id}`}
            aria-label={task.enabled ? 'Disable task' : 'Enable task'}
            checked={task.enabled}
            onCheckedChange={v =>
              void Stores.ScheduledTasks.setEnabled(task.id, v)
            }
          />
          <Button
            data-testid={`task-run-now-${task.id}`}
            variant="ghost"
            aria-label="Run now"
            onClick={async () => {
              await Stores.ScheduledTasks.runNow(task.id)
              message.info(
                'Running now — result will land in your notifications',
              )
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
