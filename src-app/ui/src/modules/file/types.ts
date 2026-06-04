import type { StoreProxy } from '@/core/stores'
import type { useFileStore } from './stores/File.store'
import type { useFilePreviewDrawerStore } from './stores/FilePreviewDrawer.store'
import type { useProjectFilesStore } from './project-extension/stores/ProjectFiles.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
    FilePreviewDrawer: StoreProxy<
      ReturnType<typeof useFilePreviewDrawerStore.getState>
    >
    ProjectFiles: StoreProxy<ReturnType<typeof useProjectFilesStore.getState>>
  }
}

export {}
