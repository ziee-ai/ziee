import type { StoreProxy } from '@/core/stores'
import type { useFileStore } from './stores/File.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
  }
}

export {}
