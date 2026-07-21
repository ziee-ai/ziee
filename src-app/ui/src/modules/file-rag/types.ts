import type { StoreProxy } from '@ziee/framework/stores'
import type { useFileRagAdminStore } from './stores/fileRagAdmin'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    FileRagAdmin: StoreProxy<ReturnType<typeof useFileRagAdminStore.getState>>
  }
}

export {}
