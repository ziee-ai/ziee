import type { StoreSet } from '@ziee/framework/store-kit'

export const onboardingState = {
  // Wizard UI state
  nextEnabled: true,
  nextLoading: false,
  nextError: null as string | null,
  // Per-user progress (owned here, not on Stores.Auth.user). `loaded` gates
  // the redirect so it can't mis-fire before the first fetch.
  completedGuideIds: [] as string[],
  completedStepIds: [] as string[],
  loading: false,
  loaded: false,
}

export type OnboardingState = typeof onboardingState
export type OnboardingSet = StoreSet<OnboardingState>
export type OnboardingGet = () => OnboardingState
