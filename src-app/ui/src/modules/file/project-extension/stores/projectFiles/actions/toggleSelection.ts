import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) =>
  (fileId: string) => {
    set(state => {
      if (state.selectedFileIds.has(fileId)) {
        state.selectedFileIds.delete(fileId)
      } else {
        state.selectedFileIds.add(fileId)
      }
    })
  }
