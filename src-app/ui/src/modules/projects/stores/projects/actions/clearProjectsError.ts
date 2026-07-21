import type { ProjectsSet, ProjectsGet } from '../state'

export default (set: ProjectsSet, _get: ProjectsGet) => () => {
  set({ error: null })
}
