import type { StoreProxy } from '@ziee/framework/stores'
import type { useFileStore } from './stores/File.store'
import { useFilePreviewDrawerStore } from './stores/filePreviewDrawer'
import type { useFileVersionsStore } from './stores/fileVersions'
import type { usePdfHighlightStore } from './stores/PdfHighlight.store'
import type { useDeliverablesStore } from './stores/deliverables'
import type { ProjectFilesDef } from './project-extension/stores/projectFiles'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    File: StoreProxy<ReturnType<typeof useFileStore.getState>>
    FilePreviewDrawer: StoreProxy<
      ReturnType<typeof useFilePreviewDrawerStore.getState>
    >
    FileVersions: StoreProxy<ReturnType<typeof useFileVersionsStore.getState>>
    PdfHighlight: StoreProxy<ReturnType<typeof usePdfHighlightStore.getState>>
    Deliverables: StoreProxy<ReturnType<typeof useDeliverablesStore.getState>>
    ProjectFiles: StoreProxy<ReturnType<typeof ProjectFilesDef.store.getState>>
  }
}

export {}
