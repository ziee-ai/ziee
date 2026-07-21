import { ApiClient } from '@/api-client'
import type { ConversationResponse } from '@/api-client/types'
import type { ProjectsGet, ProjectsSet } from '../state'
import { emitProjectConversationAttached } from '@/modules/projects/events'

export default (_set: ProjectsSet, _get: ProjectsGet) =>
  async (
    projectId: string,
    conversationId: string,
  ): Promise<ConversationResponse> => {
    // Query the conversation's current project BEFORE updating so the event
    // carries the correct `fromProjectId`.
    const currentProject = await ApiClient.Project.forConversation({
      conversation_id: conversationId,
    })
    const fromProjectId = currentProject?.id ?? null
    // API call + event only. The chat extension patches chat-side state.
    const response = await ApiClient.Project.attachConversation({
      id: projectId,
      conversation_id: conversationId,
    })
    await emitProjectConversationAttached(projectId, conversationId, fromProjectId)
    return response
  }
