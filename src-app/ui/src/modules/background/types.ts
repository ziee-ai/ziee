import type { StoreProxy } from '@ziee/framework/stores'

import type { useBackgroundRunsStore } from './stores/BackgroundRuns.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    BackgroundRuns: StoreProxy<
      ReturnType<typeof useBackgroundRunsStore.getState>
    >
  }
}

export {}
