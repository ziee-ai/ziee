import { Stores } from '@ziee/framework/stores'
import { getViewer } from '@/modules/file/registry/fileViewerRegistry'
import { InlineFilePreview } from './InlineFilePreview'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import type {
  MessageContent,
  MessageContentDataToolResult,
  ResourceLink,
  File as FileEntity,
} from '@/api-client/types'
import type { InlineFileSource } from '@/modules/file/types/viewer'

/**
 * Pull the `resource_links` carried by a `tool_result` content block. The
 * generated `MessageContentDataToolResult` already types this field
 * (`resource_links?: ResourceLink[] | null`); the cast is only because
 * `MessageContent.content` is the loosely-typed union and we've already
 * narrowed on `content_type === 'tool_result'`.
 */
function extractResourceLinks(content: MessageContent): ResourceLink[] {
  if (content.content_type !== 'tool_result') return []
  const data = content.content as MessageContentDataToolResult
  if (!Array.isArray(data.resource_links)) return []
  // Drop entries with missing/empty URIs so we don't render an
  // `<img src="">` or an open-in-new-tab link to nowhere. Backend
  // shouldn't emit these but external MCP servers might.
  return data.resource_links.filter(
    link => typeof link?.uri === 'string' && link.uri.trim().length > 0,
  )
}

/**
 * Convert a resource_link to the `InlineFileSource` shape viewers expect.
 * Filenames fall back to the URI tail when the backend didn't send a name.
 */
function toInlineSource(link: ResourceLink): InlineFileSource {
  const name =
    link.name ||
    // URI tail (after the last `/`), strip query string for display
    link.uri.split('?')[0].split('/').filter(Boolean).pop() ||
    'untitled'
  return {
    url: link.uri,
    name,
    mimeType: link.mime_type ?? undefined,
    size: link.size ?? undefined,
    fileId: link.file_id ?? undefined,
    versionId: link.version_id ?? undefined,
    version: link.version ?? undefined,
  }
}

/**
 * Build a stub `FileEntity` from a resource_link's metadata, shown while the
 * FileStore fetches the full entity. Mirrors `FileAttachmentRenderer`'s
 * fallback so backend-owned inline previews render through the same
 * authenticated `{file}` path the right-side panel uses.
 */
function buildFallbackFile(fileId: string, source: InlineFileSource): FileEntity {
  return {
    id: fileId,
    filename: source.name,
    file_size: source.size ?? 0,
    mime_type: source.mimeType ?? undefined,
    has_thumbnail: false,
    preview_page_count: 0,
    created_at: '',
    updated_at: '',
    user_id: '',
    created_by: '',
    processing_metadata: null,
    text_page_count: 0,
    version: source.version ?? 1,
    current_version_id: source.versionId ?? '',
    blob_version_id: source.versionId ?? fileId,
  }
}

/**
 * Inline files view for a single `tool_result` content block: renders every
 * `resource_link` that block carries, in place, right after the tool-call
 * card — instead of aggregating all of a message's files into a footer.
 * Registered by the file extension as the `tool_result` content-type
 * renderer (the MCP extension owns the `tool_use` card; this owns the files).
 *
 * Dedupe is per-block (a file referenced twice in the SAME tool_result renders
 * once); files returned by different tool_results render at each tool's
 * position. The inline-vs-header-only decision is owned by each viewer module
 * via its `entry.inline` field — this component never inspects MIME types.
 *
 * Returns `null` when the block has no resource_links so non-file tool_results
 * (and every other content type) add no DOM.
 */
export function MessageFilesView({ content }: ContentRendererProps) {
  const links = extractResourceLinks(content)
  if (links.length === 0) return null

  // Dedupe by URI within this block, preserving first-seen order.
  const seen = new Set<string>()
  const deduped: ResourceLink[] = []
  for (const link of links) {
    if (seen.has(link.uri)) continue
    seen.add(link.uri)
    deduped.push(link)
  }

  // Reactive subscription — re-renders when getMessageFile() resolves the
  // full entity (e.g. once a thumbnail becomes available).
  const messageFilesCache = Stores.File.messageFilesCache

  return (
    <div
      data-testid="tool-result-files"
      className="flex flex-col gap-2 mt-2 w-full"
    >
      {deduped.map(link => {
        const source = toInlineSource(link)
        // Backend-owned artifact: resolve the File entity so the body renders
        // via the authenticated `{file}` path and the side-panel button works.
        let file: FileEntity | undefined
        if (source.fileId) {
          const fallback = buildFallbackFile(source.fileId, source)
          file = messageFilesCache.get(source.fileId) ?? fallback
          // Deferred inside the store action — safe to call during render.
          Stores.File.getMessageFile(source.fileId, fallback)
        }
        const viewer = file
          ? getViewer(file.filename, file.mime_type ?? undefined)
          : getViewer(source.name, source.mimeType)
        return (
          <InlineFilePreview
            key={link.uri}
            viewer={viewer}
            source={source}
            file={file}
          />
        )
      })}
    </div>
  )
}
