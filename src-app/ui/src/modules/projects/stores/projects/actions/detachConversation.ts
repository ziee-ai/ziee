import { ApiClient } from '@/api-client'
import type { ProjectsSet, ProjectsGet } from '../state'
import { emitProjectConversationDetached } from '@/modules/projects/events'

export default (_set: ProjectsSet, _get: ProjectsGet) =>
  async (projectId: string, conversationId: string): Promise<void> => {
    await ApiClient.Project.detachConversation({
      id: projectId,
      conversation_id: conversationId,
    })
    await emitProjectConversationDetached(projectId, conversationId)
  }
