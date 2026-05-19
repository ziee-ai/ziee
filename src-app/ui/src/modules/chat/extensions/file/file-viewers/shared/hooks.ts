import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'

/**
 * Read the cached text contents for a file, triggering an async load
 * if not yet cached. Returns `null` while loading.
 *
 * Note: this calls a store action during render, which is the existing
 * convention in this codebase — the action is internally deferred and
 * safe to invoke from render.
 */
export function useFileTextContent(file: FileEntity): string | null {
  const content = Stores.Chat.FileStore.fileTextContents.get(file.id) ?? null
  if (content === null) Stores.Chat.FileStore.getFileTextContent(file.id, file)
  return content
}

/** Current view mode for a file ('compiled' default). */
export function useFileViewMode(fileId: string): 'compiled' | 'raw' {
  return Stores.Chat.FileStore.fileViewModes.get(fileId) ?? 'compiled'
}
