import type { StoreProxy } from '@/core/stores'
import type { useFileStore } from './stores/File.store'
import type { useFilePreviewDrawerStore } from './stores/FilePreviewDrawer.store'
import type { useFileVersionsStore } from './stores/FileVersions.store'
import type { ProjectFiles } from './project-extension/stores/ProjectFiles.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
    FilePreviewDrawer: StoreProxy<
      ReturnType<typeof useFilePreviewDrawerStore.getState>
    >
    FileVersions: StoreProxy<ReturnType<typeof useFileVersionsStore.getState>>
    ProjectFiles: StoreProxy<ReturnType<typeof ProjectFiles.store.getState>>
  }
}

export {}
