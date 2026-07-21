import type { ApiKeysStepSet, ApiKeysStepGet } from '../state'

export default (set: ApiKeysStepSet, _get: ApiKeysStepGet) => () => {
  set(draft => {
    draft.enteredApiKeys = {}
    // providers/userKeys/loading/error intentionally NOT reset — API cache;
    // init won't re-trigger after reset, so clearing them would blank the
    // next visit.
  })
}
