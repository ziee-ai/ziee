// Global file-preview drawer state (Stores.FilePreviewDrawer).
//
// The drawer is the universal "click a file card to preview" surface. Chat
// surfaces opt into the right-panel instead by passing an explicit onClick to
// FileCard; the drawer is FileCard's default fallback so any non-chat surface
// gets a working preview without per-surface plumbing.

import type { File as FileEntity } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const FilePreviewDrawer = defineStore('FilePreviewDrawer', {
  state: { isOpen: false, file: null as FileEntity | null },
  actions: set => ({
    openPreview: (file: FileEntity) => set({ isOpen: true, file }),
    // Keep `file` set so the exit animation has content while sliding out;
    // the next openPreview overwrites it.
    closePreview: () => set({ isOpen: false }),
  }),
})

export const useFilePreviewDrawerStore = FilePreviewDrawer.store
