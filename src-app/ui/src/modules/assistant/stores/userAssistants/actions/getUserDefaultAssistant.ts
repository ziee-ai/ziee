import type { UserAssistantsGet, UserAssistantsSet } from '../state'

export default (_set: UserAssistantsSet, get: UserAssistantsGet) => {
  return () => {
    return get().assistants.find(a => a.is_default)
  }
}
