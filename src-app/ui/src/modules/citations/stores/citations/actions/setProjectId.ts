import type { CitationsSet } from '../state'

export default (set: CitationsSet, _get: () => never) => {
  return (id: string | null) => {
    set(s => {
      s.projectId = id
    })
  }
}
