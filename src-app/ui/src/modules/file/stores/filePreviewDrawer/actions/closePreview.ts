import type { FilePreviewDrawerSet, FilePreviewDrawerGet } from '../state'
import { setOverlayOpen } from '@/core/overlays/overlayVisibility'

export default (set: FilePreviewDrawerSet, _get: FilePreviewDrawerGet) => {
  return async () => {
    set({ isOpen: false })
    setOverlayOpen('file-preview', false)
  }
}
