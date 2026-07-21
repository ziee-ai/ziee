import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Async action: fetches binary content and stores in fileBinaryContents. */
export default (set: FileSet, get: FileGet) => async (fileId: string, fallbackFile?: FileEntity) => {
  if (get().fileBinaryLoadingSet.has(fileId) || get().fileBinaryContents.has(fileId)) return

  const file = get().messageFilesCache.get(fileId) ?? get().selectedFiles.get(fileId) ?? fallbackFile
  if (!file) return

  set((state) => {
    const newSet = new Set(state.fileBinaryLoadingSet)
    newSet.add(fileId)
    state.fileBinaryLoadingSet = newSet
  })

  try {
    const response = await ApiClient.File.download({ file_id: file.id })
    const blob = response instanceof Blob ? response : new Blob([response])
    const buffer = await blob.arrayBuffer()
    set((state) => {
      const newContents = new Map(state.fileBinaryContents)
      newContents.set(fileId, buffer)
      const newSet = new Set(state.fileBinaryLoadingSet)
      newSet.delete(fileId)
      state.fileBinaryContents = newContents
      state.fileBinaryLoadingSet = newSet
    })
  } catch (error) {
    set((state) => {
      const newSet = new Set(state.fileBinaryLoadingSet)
      newSet.delete(fileId)
      state.fileBinaryLoadingSet = newSet
    })
    console.error('[FileStore] Failed to load file binary content:', error)
  }
}
