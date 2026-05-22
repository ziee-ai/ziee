import type { StoreProxy } from '@/core/stores'
import type { useOnboardingStore } from './stores/Onboarding.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Onboarding: StoreProxy<ReturnType<typeof useOnboardingStore.getState>>
  }
}

export {}
