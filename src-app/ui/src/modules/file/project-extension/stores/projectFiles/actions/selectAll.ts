import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) => () => {
  set(state => {
    for (const file of state.files) state.selectedFileIds.add(file.id)
  })
}
