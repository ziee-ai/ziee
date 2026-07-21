import type { UserAssistantsGet, UserAssistantsSet } from '../state'

export default (set: UserAssistantsSet, _get: UserAssistantsGet) => {
  return async () => {
    set(s => {
      s.error = null
    })
  }
}
