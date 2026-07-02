import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Workflow } from '@/api-client/types'
import { ApiClient } from '@/api-client'

interface GroupWorkflows {
  groupId: string
  workflows: Workflow[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface GroupSystemWorkflowsWidgetState {
  // Map of groupId -> assigned system workflows
  groupWorkflows: Map<string, GroupWorkflows>

  loadWorkflowsForGroup: (groupId: string, force?: boolean) => Promise<void>
  updateGroupWorkflows: (groupId: string, workflowIds: string[]) => Promise<void>
}

/**
 * Group → assigned system workflows, single-call per the LLM widget pattern
 * (`ApiClient.Group.getSystemWorkflows`), with 30s caching. No event
 * subscriptions: the drawer's save calls `updateGroupWorkflows` which stores
 * the returned set directly.
 */
export const useGroupSystemWorkflowsWidgetStore =
  create<GroupSystemWorkflowsWidgetState>()(
    subscribeWithSelector(
      immer((set, get): GroupSystemWorkflowsWidgetState => ({
        groupWorkflows: new Map(),

        loadWorkflowsForGroup: async (groupId, force = false): Promise<void> => {
          const existing = get().groupWorkflows.get(groupId)
          if (existing?.loading && !force) return
          if (
            !force &&
            existing?.lastFetched &&
            Date.now() - existing.lastFetched < 30000 &&
            !existing.error
          ) {
            return
          }

          set(state => {
            state.groupWorkflows.set(groupId, {
              groupId,
              workflows: existing?.workflows ?? [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched ?? null,
            })
          })

          try {
            const response = await ApiClient.Group.getSystemWorkflows({
              group_id: groupId,
            })
            set(state => {
              state.groupWorkflows.set(groupId, {
                groupId,
                workflows: response.workflows,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(
              `Failed to load workflows for group ${groupId}:`,
              error,
            )
            set(state => {
              state.groupWorkflows.set(groupId, {
                groupId,
                workflows: existing?.workflows ?? [],
                loading: false,
                error:
                  error instanceof Error
                    ? error.message
                    : 'Failed to load workflows',
                lastFetched: existing?.lastFetched ?? null,
              })
            })
          }
        },

        updateGroupWorkflows: async (groupId, workflowIds): Promise<void> => {
          const response = await ApiClient.Group.updateSystemWorkflows({
            group_id: groupId,
            workflow_ids: workflowIds,
          })
          set(state => {
            state.groupWorkflows.set(groupId, {
              groupId,
              workflows: response.workflows,
              loading: false,
              error: null,
              lastFetched: Date.now(),
            })
          })
        },
      })),
    ),
  )
