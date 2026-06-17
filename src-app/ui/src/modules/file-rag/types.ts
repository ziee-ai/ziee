import type { StoreProxy } from '@/core/stores'
import type { useFileRagAdminStore } from './stores/FileRagAdmin.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    FileRagAdmin: StoreProxy<ReturnType<typeof useFileRagAdminStore.getState>>
  }
}

export {}
