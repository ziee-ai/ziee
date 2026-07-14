import { createModule } from '@ziee/framework'
import { Stores } from '@ziee/framework/stores'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { useFileStore } from './stores/File.store'
import { useFilePreviewDrawerStore } from './stores/FilePreviewDrawer.store'
import { useFileVersionsStore } from './stores/FileVersions.store'
import { useDeliverablesStore } from './stores/Deliverables.store'
import { ProjectFiles } from './project-extension/stores/ProjectFiles.store'
import { PdfHighlight } from './stores/PdfHighlight.store'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'
// Side-effect import — registers the file knowledge kind into the
// projectExtensionRegistry. The projects module's auto-discovery glob
// also picks this up (via `modules/*/project-extension/extension.tsx`),
// but importing here ensures the registration runs even when the file
// module loads before the projects glob.
import './project-extension/extension'
// Augments AppEvents with project.file_attached/detached event types
// (relocated from projects/events as part of the project↔file inversion).
import './project-extension/events/types'

const FilePreviewDrawer = lazyWithPreload(() =>
  import('./components/FilePreviewDrawer').then(m => ({
    default: m.FilePreviewDrawer,
  })),
)

const FileViewPage = lazyWithPreload(() =>
  import('./components/FileViewPage').then(m => ({
    default: m.FileViewPage,
  })),
)

/**
 * File module — top-level home for file-domain state, components,
 * viewers, and the file-viewer registry. One route (the dedicated
 * full-page file view at /files/:fileId), no nav slots, no admin
 * pages: file is a chat-composer concern (chat-extension auto-
 * discovered at modules/file/chat-extension/) AND a cross-module
 * primitive (projects' knowledge drawer reuses FileCard at
 * modules/file/components/FileCard.tsx).
 *
 * Backend counterpart: modules/file/ + modules/file/chat_extension/.
 */
export default createModule({
  metadata: {
    name: 'file',
    version: '1.0.0',
    description: 'File storage, upload, preview and viewer registry',
  },
  dependencies: ['router'],
  routes: [
    {
      // Dedicated full-page file view — reached via the FullPageButton in the
      // viewer chrome (opens /files/:fileId, closing the preview drawer).
      path: '/files/:fileId',
      element: FileViewPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
  ],
  stores: [
    { name: 'File', store: useFileStore },
    { name: 'FilePreviewDrawer', store: useFilePreviewDrawerStore },
    { name: 'FileVersions', store: useFileVersionsStore },
    { name: 'Deliverables', store: useDeliverablesStore },
    // defineStore handle already carries its { name, store } — name once.
    ProjectFiles,
    PdfHighlight,
  ],
  components: [
    {
      // Global file-preview drawer — FileCard's default click opens
      // this so any non-chat surface (project knowledge drawer,
      // knowledge card on ProjectDetailPage, etc.) gets preview
      // without per-surface plumbing. Chat surfaces opt into the
      // side-by-side right-panel via explicit onClick instead.
      id: 'file-preview-drawer',
      component: FilePreviewDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.FilePreviewDrawer.isOpen),
      order: 50,
    },
  ],
})
