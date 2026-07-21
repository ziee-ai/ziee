import type { ProjectDetailSet } from '../state'

export default (set: ProjectDetailSet) => () => {
  set({ error: null })
}
