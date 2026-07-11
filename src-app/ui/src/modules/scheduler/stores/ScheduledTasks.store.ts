import { ApiClient } from '@/api-client'
import {
  type CreateScheduledTask,
  Permissions,
  type ScheduledTask,
  type ScheduledTaskRun,
  type TestFireRequest,
  type TestFireResult,
  type UpdateScheduledTask,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

import { RUNS_PAGE_SIZE } from '../components/runTimeline'

interface RunsMeta {
  total: number
  page: number
  perPage: number
}

/**
 * The scheduled-tasks store: list + CRUD + run-now + test-fire + per-task run
 * history. Subscribes to `sync:scheduled_task` (+ `sync:reconnect`) to refetch
 * live. Self-gates every fetch on `SchedulerUse` (no-403 invariant).
 */
export const ScheduledTasks = defineStore('ScheduledTasks', {
  immer: true,
  state: {
    tasks: [] as ScheduledTask[],
    loading: false,
    error: null as string | null,
    runsByTask: {} as Record<string, ScheduledTaskRun[]>,
    runsMetaByTask: {} as Record<string, RunsMeta>,
    runsLoading: false,
  },
  actions: set => {
    const loadTasks = async () => {
      if (!hasPermissionNow(Permissions.SchedulerUse)) return
      set(draft => {
        draft.loading = true
        draft.error = null
      })
      try {
        const tasks = await ApiClient.ScheduledTask.list()
        set(draft => {
          draft.tasks = tasks
          draft.loading = false
        })
      } catch (error) {
        set(draft => {
          draft.loading = false
          draft.error =
            error instanceof Error
              ? error.message
              : 'Failed to load scheduled tasks'
        })
      }
    }

    return {
      loadTasks,
      createTask: async (body: CreateScheduledTask): Promise<ScheduledTask> => {
        const task = await ApiClient.ScheduledTask.create(body)
        set(draft => {
          draft.tasks.unshift(task)
        })
        return task
      },
      updateTask: async (
        id: string,
        patch: UpdateScheduledTask,
      ): Promise<ScheduledTask> => {
        const task = await ApiClient.ScheduledTask.update({ id, ...patch })
        set(draft => {
          const i = draft.tasks.findIndex(t => t.id === id)
          if (i >= 0) draft.tasks[i] = task
        })
        return task
      },
      setEnabled: async (id: string, enabled: boolean) => {
        const task = await ApiClient.ScheduledTask.update({ id, enabled })
        set(draft => {
          const i = draft.tasks.findIndex(t => t.id === id)
          if (i >= 0) draft.tasks[i] = task
        })
      },
      deleteTask: async (id: string) => {
        await ApiClient.ScheduledTask.delete({ id })
        set(draft => {
          draft.tasks = draft.tasks.filter(t => t.id !== id)
        })
      },
      runNow: async (id: string) => {
        await ApiClient.ScheduledTask.runNow({ id })
      },
      continueRun: async (runId: string): Promise<string> => {
        const res = await ApiClient.ScheduledTask.continueRun({ run_id: runId })
        return res.conversation_id
      },
      // ITEM-43/47 (DEC-22): seed a conversation with the last N runs for a
      // series-level discussion; returns the new conversation id to navigate to.
      continueSeries: async (
        taskId: string,
        limit: number,
      ): Promise<string> => {
        const res = await ApiClient.ScheduledTask.continueSeries({
          id: taskId,
          limit,
        })
        return res.conversation_id
      },
      testFire: async (req: TestFireRequest): Promise<TestFireResult> => {
        return ApiClient.ScheduledTask.testFire(req)
      },
      // ITEM-41/46: load ONE page of run history (default page 1). Stores the
      // paged slice + total so the panel can render `ListPagination`.
      loadRuns: async (taskId: string, page = 1) => {
        if (!hasPermissionNow(Permissions.SchedulerUse)) return
        set(draft => {
          draft.runsLoading = true
        })
        try {
          const res = await ApiClient.ScheduledTask.listRuns({
            id: taskId,
            page,
            per_page: RUNS_PAGE_SIZE,
          })
          set(draft => {
            draft.runsByTask[taskId] = res.runs
            draft.runsMetaByTask[taskId] = {
              total: res.total,
              page: res.page,
              perPage: res.per_page,
            }
            draft.runsLoading = false
          })
        } catch {
          set(draft => {
            draft.runsLoading = false
          })
        }
      },
      clearError: () =>
        set(draft => {
          draft.error = null
        }),
    }
  },
  init: ({ on, get, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.SchedulerUse)) return
      void actions.loadTasks()
      // refresh the open task's runs, if any are loaded — refetch the SAME page.
      const meta = get().runsMetaByTask
      const loaded = Object.keys(get().runsByTask)
      for (const id of loaded) void actions.loadRuns(id, meta[id]?.page ?? 1)
    }
    on('sync:scheduled_task', reload)
    on('sync:reconnect', reload)
  },
})

export const useScheduledTasksStore = ScheduledTasks.store
