import type { ProjectDrawerGet, ProjectDrawerSet } from '../state'

export default (set: ProjectDrawerSet, _get: ProjectDrawerGet) =>
  async (loading: boolean) => {
    set({ loading })
  }
