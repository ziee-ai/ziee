import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { projectDetailState, type ProjectDetailState } from './state'
import type { Actions } from './actions.gen'

const ProjectDetailDef = defineStore<ProjectDetailState, Actions>('ProjectDetail', {
  immer: true,
  state: projectDetailState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    // Refresh the currently-loaded project when it changes upstream.
    on('project.updated', event => {
      const current = get().project
      if (current && current.id === event.data.project.id) {
        set({ project: event.data.project })
      }
    })
    // Cross-device: a remote edit arrives as a `sync:project` frame (the local
    // `project.updated` only fires for same-device mutations). Refetch the open
    // project. Self-gated per the no-403-reconnect convention.
    const reloadOnSync = () => {
      if (!hasPermissionNow(Permissions.ProjectsRead)) return
      const id = get().project?.id
      if (id) void actions.loadProject(id)
    }
    on('sync:project', reloadOnSync)
    on('sync:reconnect', reloadOnSync)
    // Drop a conversation from the list when ANY component deletes it.
    on('conversation.deleted', event => {
      set(state => {
        state.conversations = state.conversations.filter(
          c => c.id !== event.data.conversationId,
        )
      })
    })
    // Detaching a conversation from THIS project drops it from the list.
    on('project.conversation_detached', event => {
      if (event.data.projectId !== get().project?.id) return
      set(state => {
        state.conversations = state.conversations.filter(
          c => c.id !== event.data.conversationId,
        )
      })
    })
  },
})
export { ProjectDetailDef }
export const ProjectDetail = registerLazyStore(ProjectDetailDef)
export const useProjectDetailStore = ProjectDetailDef.store
