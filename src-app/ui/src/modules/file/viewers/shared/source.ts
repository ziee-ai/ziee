import type { File as FileEntity } from '@/api-client/types'
import type {
  FileSupportEntry,
  FileViewerEntry,
  FileViewerSlotProps,
  InlineFileSource,
} from '../../types/viewer'

/**
 * Flat shape a viewer body works with, regardless of whether it was
 * called in the right-panel context (`{file}`) or the chat-inline
 * context (`{source}`). The `file` field is `undefined` when called
 * from inline; viewers that need rich metadata (e.g. ImageBody's
 * thumbnail-cache optimisation) branch on its presence.
 */
export interface ResolvedFileSource {
  url: string
  name: string
  mimeType?: string
  size?: number
  /** Set only in the right-panel context. */
  file?: FileEntity
}

/**
 * Narrow the discriminated-union `FileViewerSlotProps` to a flat
 * `ResolvedFileSource`. Saves every viewer from repeating the
 * `'file' in props ? ... : ...` check.
 *
 * For the `{file}` branch we synthesise the canonical download URL
 * `/api/files/{id}/download` — that's what existing right-panel
 * viewer code uses elsewhere via `FileStore.getDownloadUrl`. Viewers
 * that already pull from a more specific store (e.g. ImageBody's
 * `thumbnailUrls`) can ignore `url` and use `file` directly.
 */
export function getSource(props: FileViewerSlotProps): ResolvedFileSource {
  if ('file' in props) {
    return {
      url: `/api/files/${props.file.id}/download`,
      name: props.file.filename,
      mimeType: props.file.mime_type ?? undefined,
      size: props.file.file_size,
      file: props.file,
    }
  }
  const s: InlineFileSource = props.source
  return { url: s.url, name: s.name, mimeType: s.mimeType, size: s.size }
}

/**
 * Decide whether a viewer's `inline` declaration covers the given
 * file's MIME/ext. Mirrors the matching logic in
 * `fileViewerRegistry.ts::scoreSupport` (treating `mime: 'foo/*'`
 * as a wildcard, OR-ing `ext` and `mime` within one rule).
 *
 *  - `inline === true` → always inline-capable.
 *  - `inline === false | undefined` → never inline-capable.
 *  - `inline` is an array → inline-capable iff any rule matches.
 */
export function isInlineCapable(
  entry: FileViewerEntry | undefined,
  name: string,
  mimeType: string | undefined,
): boolean {
  if (!entry || !entry.inline) return false
  if (entry.inline === true) return true
  return entry.inline.some(rule => matchesSupport(rule, name, mimeType))
}

function matchesSupport(
  rule: FileSupportEntry,
  name: string,
  mimeType: string | undefined,
): boolean {
  if (rule.ext !== undefined) {
    const ext = name.split('.').pop()?.toLowerCase()
    if (ext === rule.ext.toLowerCase()) return true
  }
  if (rule.mime !== undefined && mimeType) {
    if (rule.mime.endsWith('/*')) {
      const prefix = rule.mime.slice(0, -1)
      if (mimeType.startsWith(prefix)) return true
    } else if (mimeType.toLowerCase() === rule.mime.toLowerCase()) {
      return true
    }
  }
  return false
}
