import type { StoreProxy } from '@ziee/framework/stores'
import type { useOnboardingStore } from './stores/onboarding'
import './events/types'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Onboarding: StoreProxy<ReturnType<typeof useOnboardingStore.getState>>
  }
}

export {}
