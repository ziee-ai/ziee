import type { FilePreviewDrawerSet, FilePreviewDrawerGet } from '../state'

export default (set: FilePreviewDrawerSet, _get: FilePreviewDrawerGet) => {
  return async () => {
    set({ isOpen: false })
  }
}
