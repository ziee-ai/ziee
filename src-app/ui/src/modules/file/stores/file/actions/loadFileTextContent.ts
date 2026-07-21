import { ApiClient } from '@/api-client'
import type { FileGet, FileSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

/** Async action: fetches text/html/svg content and stores in fileTextContents. */
export default (set: FileSet, get: FileGet) => async (fileId: string, fallbackFile?: FileEntity) => {
  if (get().fileTextLoadingSet.has(fileId) || get().fileTextContents.has(fileId)) return

  const file = get().messageFilesCache.get(fileId) ?? get().selectedFiles.get(fileId) ?? fallbackFile
  if (!file) return

  set((state) => {
    const newSet = new Set(state.fileTextLoadingSet)
    newSet.add(fileId)
    state.fileTextLoadingSet = newSet
  })

  try {
    let text = ''
    const e = file.filename.split('.').pop()?.toLowerCase() ?? ''
    const isHtmlOrSvg =
      file.mime_type === 'text/html' || file.mime_type === 'image/svg+xml' ||
      e === 'html' || e === 'htm' || e === 'svg'
    if (isHtmlOrSvg) {
      const response = await ApiClient.File.download({ file_id: file.id })
      const blob = response instanceof Blob ? response : new Blob([response])
      text = await blob.text()
    } else {
      const response = await ApiClient.File.getTextContent({ file_id: file.id })
      text = typeof response === 'string' ? response : await (response as Blob).text()
    }
    set((state) => {
      const newContents = new Map(state.fileTextContents)
      newContents.set(fileId, text)
      const newSet = new Set(state.fileTextLoadingSet)
      newSet.delete(fileId)
      state.fileTextContents = newContents
      state.fileTextLoadingSet = newSet
    })
  } catch (error) {
    set((state) => {
      const newSet = new Set(state.fileTextLoadingSet)
      newSet.delete(fileId)
      state.fileTextLoadingSet = newSet
    })
    console.error('[FileStore] Failed to load file text content:', error)
  }
}
