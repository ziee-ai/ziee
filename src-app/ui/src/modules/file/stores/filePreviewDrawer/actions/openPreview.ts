import type { File as FileEntity } from '@/api-client/types'
import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { FilePreviewDrawerSet, FilePreviewDrawerGet } from '../state'

// Keep `file` set so the exit animation has content while sliding out;
// the next openPreview overwrites it.
export default (set: FilePreviewDrawerSet, _get: FilePreviewDrawerGet) => {
  return async (file: FileEntity) => {
    set({ isOpen: true, file })
    setOverlayOpen('file-preview', true)
  }
}
