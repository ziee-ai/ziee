import { Stores } from '@ziee/framework/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { FileCard } from '@/modules/file/components/FileCard'
import type { File as FileEntity } from '@/api-client/types'
import { File as FileStore } from '@/modules/file/stores/file'

export interface AttachedFileCardProps {
  fileId: string
  filename: string
  fileSize?: number
  mimeType?: string
  version?: number
  versionId?: string
  /** User messages pin attachments as square cards; assistant messages use rows. */
  isUser: boolean
}

/**
 * A user/assistant message attachment rendered as a FileCard.
 *
 * Shared by the `file_attachment` renderer AND the `image`(file-source)
 * renderer so a user-attached image shows the SAME compact card as any other
 * file (instead of a full-width inline preview) — one card style for every
 * attachment. The store owns all async fetching; the fallback entity (built
 * from the content-block metadata) renders immediately while the full entity
 * (thumbnail-capable) resolves.
 */
export function AttachedFileCard({
  fileId,
  filename,
  fileSize,
  mimeType,
  version,
  versionId,
  isUser,
}: AttachedFileCardProps) {
  // Open into THIS pane's right panel (ITEM-36), not the focused pane's.
  const chat = (useChatPaneOrNull()?.store ?? Stores.Chat) as typeof Stores.Chat
  const fallback: FileEntity = {
    id: fileId,
    filename,
    file_size: fileSize ?? 0,
    mime_type: mimeType ?? undefined,
    has_thumbnail: false,
    preview_page_count: 0,
    created_at: '',
    updated_at: '',
    user_id: '',
    created_by: '',
    processing_metadata: null,
    text_page_count: 0,
    version: version ?? 1,
    current_version_id: versionId ?? '',
    blob_version_id: versionId ?? fileId,
  }

  // Reactive subscription to messageFilesCache — re-renders when the file
  // entity (with thumbnail) loads.
  const messageFilesCache = FileStore.messageFilesCache
  const file = messageFilesCache.get(fileId) ?? fallback

  // Background load on first access (deferred inside the store action — safe in render).
  FileStore.getMessageFile(fileId, fallback)

  // Chat surfaces open the side-by-side right panel (mounted in ConversationPage);
  // without this, FileCard falls back to the global preview drawer.
  const openInRightPanel = () => {
    chat.displayInRightPanel({
      id: file.id,
      title: file.filename,
      type: 'file',
      // Pin the panel to the version this message referenced, so an old message
      // opens the file as it was when sent.
      data: { fileId: file.id, version: version ?? undefined },
    })
  }

  return (
    <FileCard
      file={file}
      variant={isUser ? 'square' : 'row'}
      showFileName={true}
      canRemove={false}
      canDelete={false}
      onClick={openInRightPanel}
    />
  )
}
