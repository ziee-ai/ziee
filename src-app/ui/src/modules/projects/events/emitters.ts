import type { Project } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitProjectCreated = async (project: Project) => {
  await EventBus.emit({
    type: 'project.created',
    data: { project },
  })
}

export const emitProjectUpdated = async (project: Project) => {
  await EventBus.emit({
    type: 'project.updated',
    data: { project },
  })
}

export const emitProjectDeleted = async (projectId: string) => {
  await EventBus.emit({
    type: 'project.deleted',
    data: { projectId },
  })
}

// `emitProjectFileAttached` + `emitProjectFileDetached` relocated to
// `modules/file/project-extension/events/emitters.ts` as part of the
// project↔file inversion.

export const emitProjectConversationAttached = async (
  projectId: string,
  conversationId: string,
  fromProjectId: string | null,
) => {
  await EventBus.emit({
    type: 'project.conversation_attached',
    data: { projectId, conversationId, fromProjectId },
  })
}

export const emitProjectConversationDetached = async (
  projectId: string,
  conversationId: string,
) => {
  await EventBus.emit({
    type: 'project.conversation_detached',
    data: { projectId, conversationId },
  })
}
