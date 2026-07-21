import type { ProjectHostMountsSet } from '../state'

export default (set: ProjectHostMountsSet) => () => {
  set(s => {
    s.error = null
  })
}
