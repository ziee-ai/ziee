import type { StoreProxy } from '@/core/stores'
import type { useOnboardingScreenStore } from './stores/OnboardingScreen.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    OnboardingScreen: StoreProxy<ReturnType<typeof useOnboardingScreenStore.getState>>
  }
}

export {}
