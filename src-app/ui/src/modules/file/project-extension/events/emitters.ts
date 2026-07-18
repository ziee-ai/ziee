// Emitters for project↔file events. Relocated from
// `modules/projects/events/emitters.ts`.

import { Stores } from '@ziee/framework/stores'

export const emitProjectFileAttached = async (
  projectId: string,
  fileId: string,
) => {
  await Stores.EventBus.emit({
    type: 'project.file_attached',
    data: { projectId, fileId },
  })
}

export const emitProjectFileDetached = async (
  projectId: string,
  fileId: string,
) => {
  await Stores.EventBus.emit({
    type: 'project.file_detached',
    data: { projectId, fileId },
  })
}
