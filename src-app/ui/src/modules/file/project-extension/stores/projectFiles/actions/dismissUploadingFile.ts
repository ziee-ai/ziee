import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) =>
  (uploadId: string) => {
    set(state => {
      state.uploadingFiles.delete(uploadId)
    })
  }
