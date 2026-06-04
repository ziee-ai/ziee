// Global file-preview drawer state.
//
// The drawer is the universal "click a file card to preview" surface.
// Chat surfaces opt into the side-by-side right-panel (`displayInRightPanel`)
// instead by passing an explicit `onClick` to FileCard — the drawer is
// FileCard's default fallback so any non-chat surface (project knowledge
// drawer, knowledge card on ProjectDetailPage, future file library page,
// etc.) gets a working preview without per-surface plumbing.
//
// Exposed as `Stores.FilePreviewDrawer`.

import { create } from 'zustand'
import type { File as FileEntity } from '@/api-client/types'

interface FilePreviewDrawerState {
  isOpen: boolean
  file: FileEntity | null

  openPreview: (file: FileEntity) => void
  closePreview: () => void
}

export const useFilePreviewDrawerStore = create<FilePreviewDrawerState>()(
  (set): FilePreviewDrawerState => ({
    isOpen: false,
    file: null,

    openPreview: (file: FileEntity) => {
      set({ isOpen: true, file })
    },

    closePreview: () => {
      // Keep `file` set so antd's exit animation has content to render
      // while the drawer slides out. Next openPreview overwrites it.
      set({ isOpen: false })
    },
  }),
)
