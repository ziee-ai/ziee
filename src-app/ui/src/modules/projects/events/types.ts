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

export type ProjectModuleEvent =
  | ProjectCreatedEvent
  | ProjectUpdatedEvent
  | ProjectDeletedEvent
  | ProjectFileAttachedEvent
  | ProjectFileDetachedEvent

declare module '@/core/events' {
  interface AppEvents {
    'project.created': ProjectCreatedEvent
    'project.updated': ProjectUpdatedEvent
    'project.deleted': ProjectDeletedEvent
    'project.file_attached': ProjectFileAttachedEvent
    'project.file_detached': ProjectFileDetachedEvent
  }
}
