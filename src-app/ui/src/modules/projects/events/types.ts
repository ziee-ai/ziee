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

// `ProjectFileAttachedEvent` + `ProjectFileDetachedEvent` relocated to
// `modules/file/project-extension/events/types.ts` as part of the
// project↔file inversion. Event NAMES ("project.file_attached" /
// "project.file_detached") are preserved verbatim — subscribers
// unaffected; only the declaration site moves.

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
  | ProjectConversationAttachedEvent
  | ProjectConversationDetachedEvent

declare module '@/core/events' {
  interface AppEvents {
    'project.created': ProjectCreatedEvent
    'project.updated': ProjectUpdatedEvent
    'project.deleted': ProjectDeletedEvent
    'project.conversation_attached': ProjectConversationAttachedEvent
    'project.conversation_detached': ProjectConversationDetachedEvent
  }
}
