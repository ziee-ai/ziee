import type { OnboardingGet, OnboardingSet } from '../state'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async (error: string | null) => {
    set(draft => {
      draft.nextError = error
    })
  }
