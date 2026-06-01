import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { Stores } from '@/core/stores'
import { getViewer } from '@/modules/chat/extensions/file/fileViewerRegistry'
import { InlineFilePreview } from './InlineFilePreview'
import type { MessageContent, File as FileEntity } from '@/api-client/types'
import type { InlineFileSource } from '../types'

/**
 * Runtime shape of a `resource_link` carried inside a `tool_result`
 * content block. The backend's `McpContentData::ToolResult` stores
 * these in `resource_links: Option<Vec<ResourceLink>>` (see
 * `src-app/server/src/modules/chat/extensions/mcp/content.rs`), and
 * the JSONB is persisted with the field intact — but the
 * schema-facing `MessageContentDataVariants::ToolResult`
 * (`extensions/mcp/extension.rs:222-246`) doesn't include it, so the
 * generated TS type for `tool_result` content omits the field. The
 * runtime value is still there; we cast.
 */
interface RuntimeResourceLink {
  uri: string
  name?: string | null
  mime_type?: string | null
  size?: number | null
  is_saved?: boolean | null
  /** Backing File id for backend-owned artifacts (set by the MCP save
   *  pipeline). Absent for external MCP links. */
  file_id?: string | null
}

function extractResourceLinks(content: MessageContent): RuntimeResourceLink[] {
  if (content.content_type !== 'tool_result') return []
  const data = content.content as unknown as {
    resource_links?: RuntimeResourceLink[] | null
  }
  if (!Array.isArray(data.resource_links)) return []
  // Drop entries with missing/empty URIs so we don't render an
  // `<img src="">` or an open-in-new-tab link to nowhere. Backend
  // shouldn't emit these but external MCP servers might.
  return data.resource_links.filter(
    link =>
      typeof link?.uri === 'string' && link.uri.trim().length > 0,
  )
}

/**
 * Convert a runtime resource_link to the `InlineFileSource` shape
 * viewers expect. Filenames fall back to the URI tail when the
 * backend didn't send a name.
 */
function toInlineSource(link: RuntimeResourceLink): InlineFileSource {
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
  }
}

/**
 * Files-view footer: aggregates every `resource_link` referenced by
 * a tool_result block in the current message, dedupes by URI, and
 * renders each one through the file-viewer registry.
 *
 * The actual rendering decision (inline body vs. header-only file
 * card) is owned entirely by each viewer module via its
 * `entry.inline` field — this component never inspects MIME types
 * directly. See [[file-viewer-modular-system]].
 *
 * Returns `null` when the message has no tool_result blocks with
 * resource_links, so text-only messages don't grow extra DOM nodes.
 */
export function MessageFilesView() {
  const message = useMessageContext()
  if (!message) return null

  const allLinks: RuntimeResourceLink[] = []
  for (const content of message.contents ?? []) {
    for (const link of extractResourceLinks(content)) {
      allLinks.push(link)
    }
  }
  if (allLinks.length === 0) return null

  // Dedupe by URI, preserving first-seen order. Same file referenced
  // in two tool_results within one assistant turn renders once.
  const seen = new Set<string>()
  const deduped: RuntimeResourceLink[] = []
  for (const link of allLinks) {
    if (seen.has(link.uri)) continue
    seen.add(link.uri)
    deduped.push(link)
  }

  // Reactive subscription — re-renders when getMessageFile() resolves the
  // full entity (e.g. once a thumbnail becomes available).
  const messageFilesCache = Stores.Chat.FileStore.messageFilesCache

  return (
    <div
      data-testid="message-files-view"
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
          Stores.Chat.FileStore.getMessageFile(source.fileId, fallback)
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
