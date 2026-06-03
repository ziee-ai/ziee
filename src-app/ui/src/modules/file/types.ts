import type { StoreProxy } from '@/core/stores'
import type { useFileStore } from './stores/File.store'
import type { useProjectFilesStore } from './project-extension/stores/ProjectFiles.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
    ProjectFiles: StoreProxy<ReturnType<typeof useProjectFilesStore.getState>>
  }
}

export {}
