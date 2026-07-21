import type { OnboardingGet, OnboardingSet } from '../state'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async () => {
    set(draft => {
      draft.nextEnabled = true
      draft.nextLoading = false
      draft.nextError = null
    })
  }
