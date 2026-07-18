import { Spin } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { AttachedFileCard } from '@/modules/file/chat-extension/components/AttachedFileCard'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import type {
  MessageContentDataImage,
  File as FileEntity,
} from '@/api-client/types'

/**
 * Renderer for `image` message content blocks. Previously unhandled — they fell
 * through to ChatMessage's "Unknown content type: image" placeholder.
 *
 * The block's `source` is one of three shapes:
 *   - `url`    — only same-origin / relative-API URLs are shown (the SafeImg
 *                exfil guard: an external `<img src>` would leak session state).
 *   - `base64` — inline data URI, allowed only for `image/*` media types (a
 *                `data:text/html` would be an XSS vector).
 *   - `file`   — resolve the file via the store and show its authenticated
 *                preview blob (same path the inline image viewer uses).
 */

function isSameOriginUrl(url: string): boolean {
  if (!url) return false
  if (url.startsWith('/')) return true
  try {
    return new URL(url, window.location.origin).origin === window.location.origin
  } catch {
    return false
  }
}

function fallbackFile(fileId: string, alt?: string | null): FileEntity {
  return {
    id: fileId,
    filename: alt || 'image',
    file_size: 0,
    mime_type: undefined,
    has_thumbnail: false,
    preview_page_count: 0,
    created_at: '',
    updated_at: '',
    user_id: '',
    created_by: '',
    processing_metadata: null,
    text_page_count: 0,
    version: 1,
    current_version_id: '',
    blob_version_id: fileId,
  }
}

function Img({ src, alt }: { src: string; alt: string }) {
  return (
    <div className="my-2" data-testid="chat-image-content">
      <img
        src={src}
        alt={alt}
        loading="lazy"
        decoding="async"
        className="max-w-full max-h-[min(60vh,36rem)] rounded-md object-contain"
      />
    </div>
  )
}

export function ImageContent({ content, isUser }: ContentRendererProps) {
  const data = content.content as MessageContentDataImage
  const source = data.source
  const alt = data.alt_text || 'image'

  // A user-attached image is a stored file (source.type === 'file'). Render it
  // as the SAME compact FileCard every other user attachment uses — not a
  // full-width inline preview — so the attachment row is uniform (and edit
  // restores it to the composer). Assistant/tool images (url / base64, or
  // file-source images the model returned) keep the inline preview below.
  if (isUser && source.type === 'file') {
    return (
      <AttachedFileCard
        fileId={source.file_id}
        filename={data.alt_text || 'image'}
        isUser={isUser}
      />
    )
  }

  if (source.type === 'url') {
    return isSameOriginUrl(source.url) ? <Img src={source.url} alt={alt} /> : null
  }

  if (source.type === 'base64') {
    if (!source.media_type?.startsWith('image/')) return null
    return <Img src={`data:${source.media_type};base64,${source.data}`} alt={alt} />
  }

  // `file` source — resolve the entity + its authenticated preview blob URL.
  const fileId = source.file_id
  const fallback = fallbackFile(fileId, data.alt_text)
  const file = Stores.File.messageFilesCache.get(fileId) ?? fallback
  Stores.File.getMessageFile(fileId, fallback)
  // Subscribe to the blob-url Map directly so we re-render once it resolves.
  const url = Stores.File.thumbnailUrls.get(fileId) ?? null
  if (url === null) Stores.File.getThumbnailUrl(fileId, file)
  if (!url) {
    return (
      <div className="flex items-center py-4">
        <Spin label="Loading" />
      </div>
    )
  }
  return <Img src={url} alt={data.alt_text || file.filename || 'image'} />
}
