import type { ApiKeysStepSet, ApiKeysStepGet } from '../state'

export default (set: ApiKeysStepSet, _get: ApiKeysStepGet) =>
  (providerId: string, value: string) => {
    set(draft => {
      draft.enteredApiKeys[providerId] = value
    })
  }
