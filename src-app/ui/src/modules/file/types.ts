import type { StoreProxy } from '@ziee/framework/stores'
import type { useFileStore } from './stores/File.store'
import type { useFilePreviewDrawerStore } from './stores/FilePreviewDrawer.store'
import type { useFileVersionsStore } from './stores/FileVersions.store'
import type { usePdfHighlightStore } from './stores/PdfHighlight.store'
import type { useDeliverablesStore } from './stores/Deliverables.store'
import type { ProjectFiles } from './project-extension/stores/ProjectFiles.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
    FilePreviewDrawer: StoreProxy<
      ReturnType<typeof useFilePreviewDrawerStore.getState>
    >
    FileVersions: StoreProxy<ReturnType<typeof useFileVersionsStore.getState>>
    PdfHighlight: StoreProxy<ReturnType<typeof usePdfHighlightStore.getState>>
    Deliverables: StoreProxy<ReturnType<typeof useDeliverablesStore.getState>>
    ProjectFiles: StoreProxy<ReturnType<typeof ProjectFiles.store.getState>>
  }
}

export {}
