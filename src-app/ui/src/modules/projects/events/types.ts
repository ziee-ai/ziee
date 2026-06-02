import type { BaseEvent } from '@/core/events'
import type { Project } from '@/api-client/types'

export interface ProjectCreatedEvent extends BaseEvent {
  type: 'project.created'
  data: {
    project: Project
  }
}

export interface ProjectUpdatedEvent extends BaseEvent {
  type: 'project.updated'
  data: {
    project: Project
  }
}

export interface ProjectDeletedEvent extends BaseEvent {
  type: 'project.deleted'
  data: {
    projectId: string
  }
}

export interface ProjectFileAttachedEvent extends BaseEvent {
  type: 'project.file_attached'
  data: {
    projectId: string
    fileId: string
  }
}

export interface ProjectFileDetachedEvent extends BaseEvent {
  type: 'project.file_detached'
  data: {
    projectId: string
    fileId: string
  }
}

/**
 * Fired after a conversation is attached (or re-attached across
 * projects) via `POST /projects/{id}/conversations/{conv_id}`.
 * `fromProjectId` is the previous project_id (null when the
 * conversation was unfiled before this attach) so subscribers can
 * distinguish a fresh attach from a cross-project move.
 */
export interface ProjectConversationAttachedEvent extends BaseEvent {
  type: 'project.conversation_attached'
  data: {
    projectId: string
    conversationId: string
    fromProjectId: string | null
  }
}

/**
 * Fired after a conversation is detached via
 * `DELETE /projects/{id}/conversations/{conv_id}`.
 */
export interface ProjectConversationDetachedEvent extends BaseEvent {
  type: 'project.conversation_detached'
  data: {
    projectId: string
    conversationId: string
  }
}

export type ProjectModuleEvent =
  | ProjectCreatedEvent
  | ProjectUpdatedEvent
  | ProjectDeletedEvent
  | ProjectFileAttachedEvent
  | ProjectFileDetachedEvent
  | ProjectConversationAttachedEvent
  | ProjectConversationDetachedEvent

declare module '@/core/events' {
  interface AppEvents {
    'project.created': ProjectCreatedEvent
    'project.updated': ProjectUpdatedEvent
    'project.deleted': ProjectDeletedEvent
    'project.file_attached': ProjectFileAttachedEvent
    'project.file_detached': ProjectFileDetachedEvent
    'project.conversation_attached': ProjectConversationAttachedEvent
    'project.conversation_detached': ProjectConversationDetachedEvent
  }
}
