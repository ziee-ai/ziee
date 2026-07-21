import type { ProjectDrawerGet, ProjectDrawerSet } from '../state'

export default (set: ProjectDrawerSet, _get: ProjectDrawerGet) =>
  async () => {
    set({ open: false, loading: false, editingProject: null })
  }
