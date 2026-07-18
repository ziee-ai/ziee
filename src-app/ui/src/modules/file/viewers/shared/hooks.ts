import { Stores } from '@ziee/framework/stores'
import type { File as FileEntity } from '@/api-client/types'

/**
 * Read the cached text contents for a file, triggering an async load
 * if not yet cached. Returns `null` while loading.
 *
 * Pass `skip=true` to disable the hook (e.g. when this viewer is
 * being called in chat-inline context where there's no FileEntity —
 * a sibling hook like `useResourceLinkContent` handles that case).
 * No store reads happen in skip mode.
 *
 * Note: this calls a store action during render, which is the existing
 * convention in this codebase — the action is internally deferred and
 * safe to invoke from render.
 */
export function useFileTextContent(
  file: FileEntity | undefined,
  skip = false,
): string | null {
  if (skip || !file) return null
  const content = Stores.File.fileTextContents.get(file.id) ?? null
  if (content === null) Stores.File.getFileTextContent(file.id, file)
  return content
}

/** Current view mode for a file ('compiled' default). Returns 'compiled'
 *  when `fileId` is empty (skip mode for inline context). */
export function useFileViewMode(fileId: string): 'compiled' | 'raw' {
  if (!fileId) return 'compiled'
  return Stores.File.fileViewModes.get(fileId) ?? 'compiled'
}
