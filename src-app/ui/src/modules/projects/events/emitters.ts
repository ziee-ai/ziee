import { Stores } from '@/core/stores'
import type { Project } from '@/api-client/types'

export const emitProjectCreated = async (project: Project) => {
  await Stores.EventBus.emit({
    type: 'project.created',
    data: { project },
  })
}

export const emitProjectUpdated = async (project: Project) => {
  await Stores.EventBus.emit({
    type: 'project.updated',
    data: { project },
  })
}

export const emitProjectDeleted = async (projectId: string) => {
  await Stores.EventBus.emit({
    type: 'project.deleted',
    data: { projectId },
  })
}

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
