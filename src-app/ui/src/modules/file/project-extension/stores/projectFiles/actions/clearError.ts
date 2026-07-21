import type { ProjectFilesGet, ProjectFilesSet } from '../state'

export default (set: ProjectFilesSet, _get: ProjectFilesGet) => () => {
  set({ error: null })
}
