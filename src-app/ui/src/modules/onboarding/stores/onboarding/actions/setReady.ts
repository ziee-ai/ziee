import type { OnboardingGet, OnboardingSet } from '../state'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async (enabled: boolean) => {
    set(draft => {
      draft.nextEnabled = enabled
    })
  }
