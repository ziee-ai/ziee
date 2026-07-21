import type { OnboardingGet, OnboardingSet } from '../state'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async (loading: boolean) => {
    set(draft => {
      draft.nextLoading = loading
    })
  }
