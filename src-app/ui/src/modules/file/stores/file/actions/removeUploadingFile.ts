import type { FileGet, FileSet } from '../state'

/** Remove an uploading file (cancel). */
export default (set: FileSet, _get: FileGet) => (progressId: string) => {
  set((state) => {
    const newUploading = new Map(state.uploadingFiles)
    newUploading.delete(progressId)
    state.uploadingFiles = newUploading
    const newOwner = new Map(state.uploadOwner)
    newOwner.delete(progressId)
    state.uploadOwner = newOwner
  })
}
