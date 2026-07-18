// Project↔file relationship events. Relocated from
// `modules/projects/events/types.ts` as part of the project↔file
// inversion — the projects module no longer carries file-shaped event
// types. Event NAMES (`project.file_attached` / `project.file_detached`)
// are preserved verbatim so existing subscribers keep working without
// changes; only the type-declaration site moves.

import type { BaseEvent } from '@ziee/framework/events'

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

declare module '@ziee/framework/events' {
  interface AppEvents {
    'project.file_attached': ProjectFileAttachedEvent
    'project.file_detached': ProjectFileDetachedEvent
  }
}
