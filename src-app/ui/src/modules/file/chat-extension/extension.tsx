import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'
import { FilePreviewList } from '@/modules/file/chat-extension/components/FilePreviewList'
import { FileUploadArea } from '@/modules/file/chat-extension/components/FileUploadArea'
import { FilePasteHandler } from '@/modules/file/chat-extension/components/FilePasteHandler'
import { FileAttachMenuItem } from '@/modules/file/chat-extension/components/FileAttachMenuItem'
import { AttachedFileCard } from '@/modules/file/chat-extension/components/AttachedFileCard'
import { MessageFilesView } from '@/modules/file/chat-extension/components/MessageFilesView'
import { ImageContent } from '@/modules/file/chat-extension/components/ImageContent'
// Raw zustand hook for the `useSendBlocker` reactive subscription —
// going through Stores.File would fire the Stores-proxy's internal
// useEffect+useStore on property access, corrupting the outer hook
// count (see ProjectFiles.store.ts's earlier bug).
import { useFileStore } from '@/modules/file/stores/File.store'
import type { File as FileEntity, MessageContent, MessageContentDataFileAttachment, MessageContentDataImage } from '@/api-client/types'

// Module-level vars so cleanup can tear down subscriptions created in initialize.
let unsubConversation: (() => void) | null = null
let unsubEditingMessage: (() => void) | null = null

/**
 * Build a composer stub FileEntity from a message's attachment content block.
 * Handles BOTH `file_attachment` blocks and `image` blocks whose source is a
 * stored file (a user-attached image) — so editing a message with images
 * restores those images into the input, same as any other file. Returns null
 * for any other block (text / url-or-base64 images / tool results). The stub is
 * upgraded to the full entity (thumbnail-capable) by the caller.
 */
function attachmentStubFromBlock(c: MessageContent): FileEntity | null {
  if (c.content_type === 'file_attachment') {
    const data = c.content as MessageContentDataFileAttachment
    if (!data.file_id) return null
    return {
      id: data.file_id,
      filename: data.filename,
      file_size: data.file_size,
      mime_type: data.mime_type ?? undefined,
      has_thumbnail: false,
      preview_page_count: 0,
      created_at: '',
      updated_at: '',
      user_id: '',
      created_by: '',
      processing_metadata: null,
      text_page_count: 0,
      version: data.version ?? 1,
      current_version_id: data.version_id ?? '',
      blob_version_id: data.version_id ?? data.file_id,
    }
  }
  if (c.content_type === 'image') {
    const data = c.content as MessageContentDataImage
    if (data.source?.type !== 'file') return null
    const fileId = data.source.file_id
    return {
      id: fileId,
      filename: data.alt_text || 'image',
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
  return null
}

/**
 * Both composer-attached file listeners in one slot entry: the drag-and-drop
 * overlay and the paste-image handler. Each independently locates the composer
 * via `[data-chat-composer]`; grouping them lets the single `input_area_suffix`
 * slot host both without either module importing the other.
 */
function ComposerFileListeners() {
  return (
    <>
      <FileUploadArea />
      <FilePasteHandler />
    </>
  )
}

// Augment the central PanelRendererMap so `displayInRightPanel({ type: 'file',
// data: ... })` and `registerPanelRenderer('file', ...)` are type-checked.
declare module '@/modules/chat/core/stores/Chat.store' {
  interface PanelRendererMap {
    file: { fileId: string; version?: number }
  }
}

/**
 * File attachment content renderer component
 * Renders file attachments in message bubbles using FileCard.
 * Store handles all async fetching — no useState or useEffect needed here.
 */
function FileAttachmentRenderer({ content: data, isUser }: ContentRendererProps) {
  const fileData = data.content as MessageContentDataFileAttachment

  if (!fileData?.file_id || !fileData?.filename) return null

  return (
    <AttachedFileCard
      fileId={fileData.file_id}
      filename={fileData.filename}
      fileSize={fileData.file_size}
      mimeType={fileData.mime_type ?? undefined}
      version={fileData.version ?? undefined}
      versionId={fileData.version_id ?? undefined}
      isUser={isUser}
    />
  )
}

/**
 * File Extension
 * Handles file attachment upload and rendering in messages
 */
const fileExtension: ChatExtension = createExtension({
  name: 'file',
  description: 'Handles file attachment upload and rendering',
  priority: 80,

  /**
   * Subscribe to editingMessage changes in Chat.store.
   * When a message is being edited: restore its file attachments into the
   * file selection so they appear in the input area prefix (FilePreviewList).
   * When edit ends (null): clear the file selection.
   */
  initialize: async () => {
    // Register the file panel renderer so file tabs can be rendered AND
    // restored from localStorage after reload. The renderer receives the
    // serialized `data` ({ fileId }) and looks the actual File entity up
    // from FileStore at render time.
    const { registerPanelRenderer } = await import('@/modules/chat/core/stores/Chat.store')
    const { FilePanel: FilePanelComponent } = await import('@/modules/file/components/FilePanel')
    const { File: FileOutlinedIcon } = await import('lucide-react')
    const { Spin: SpinComponent } = await import('@ziee/kit')
    const { Stores: StoresRef } = await import('@ziee/framework/stores')

    registerPanelRenderer('file', {
      icon: <FileOutlinedIcon />,
      component: ({ fileId, version }) => {
        const { selectedFiles, messageFilesCache } = StoresRef.File
        const file = selectedFiles.get(fileId) ?? messageFilesCache.get(fileId) ?? null
        if (!file) return <SpinComponent label="Loading file" />
        return <FilePanelComponent file={file} initialVersion={version} />
      },
    })

    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@ziee/framework/stores')

    // Conversation-change → clear the per-composer upload buffer.
    // Replaces the implicit chat-extension-framework scoping that
    // the old Stores.Chat.FileStore used to get for free; now we wire
    // it explicitly because the store lives in the file module.
    // messageFilesCache and thumbnailUrls survive (they're keyed by
    // message/file id and used across conversations).
    unsubConversation = useChatStore.subscribe(
      state => state.conversation?.id,
      () => {
        Stores.File.clearFiles()
      },
    )

    unsubEditingMessage = useChatStore.subscribe(
      state => state.editingMessage,
      async (editingMessage) => {
        const fileStore = Stores.File
        if (!fileStore) return

        if (editingMessage) {
          // Restore attachment content blocks (file_attachment AND user-attached
          // image blocks) from the edited message into the composer.
          const stubs: FileEntity[] = editingMessage.contents
            .map(attachmentStubFromBlock)
            .filter((f): f is FileEntity => f !== null)
          if (stubs.length > 0) {
            // Phase 1 — Synchronous: stubs from block data, so selectedFiles is
            // populated before the user can click Send.
            fileStore.restoreFilesFromEdit(stubs)

            // Phase 2 — Async: upgrade stubs with full server entities (enables thumbnails).
            try {
              const fullFiles = await Promise.all(
                stubs.map(s => fileStore.getFileEntityById(s.id))
              )
              const validFiles = fullFiles.filter(Boolean) as FileEntity[]
              if (validFiles.length > 0) {
                fileStore.restoreFilesFromEdit(validFiles)
              }
            } catch (error) {
              console.error('[FileExtension] Failed to upgrade files from edit:', error)
              // Stubs are still in place — basic preview still works
            }
          }
        } else {
          // Edit ended (cancel or send) — clear the file selection
          fileStore.clearFiles()
        }
      }
    )
  },

  /**
   * Tear down subscriptions created in initialize to avoid leaking
   * zustand store listeners when the extension is unregistered.
   */
  cleanup: async () => {
    unsubConversation?.()
    unsubConversation = null
    unsubEditingMessage?.()
    unsubEditingMessage = null
  },

  /**
   * Provide file attachment content blocks for the temp user message created
   * during sendMessage(). Without this, the message bubble shows no file
   * previews until loadMessages() replaces the temp message with the real one.
   */
  provideUserContent: async (_text: string, _composedRequest: any): Promise<MessageContent[]> => {
    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File
    if (!fileStore) return []

    const files = fileStore.getFiles()
    if (files.length === 0) return []

    const now = new Date().toISOString()
    return files.map((file, index) => ({
      id: crypto.randomUUID(),
      message_id: '',
      content_type: 'file_attachment',
      content: {
        type: 'file_attachment',
        file_id: file.id,
        filename: file.filename,
        file_size: file.file_size,
        mime_type: file.mime_type ?? undefined,
        // Stamp the head version pin so the optimistic temp block matches the
        // server-persisted block (which pins head) — no flicker on replace.
        version: file.version,
        version_id: file.current_version_id,
      } as MessageContentDataFileAttachment,
      sequence_order: index + 1, // text block is at sequence_order 0
      created_at: now,
      updated_at: now,
    }))
  },

  // Called by the chat store when regenerating/editing a previously-sent
  // message — rehydrates the file selection from the message's
  // file_attachment content blocks so the next send carries them.
  //
  // Stubs are built synchronously from block data (filename/size/mime)
  // because sendMessage() fires immediately after this returns and
  // clearFiles() runs right after — an async server fetch wouldn't
  // complete in time. This matches the existing `editingMessage`
  // subscribe behavior (which handles the initial edit-click flow).
  //
  // Inverts the file-specific code that used to live at
  // Chat.store.ts:891-921 (lazy-imported Stores.File to avoid the
  // chat → file dependency).
  onMessageEditRestore: async (contents) => {
    const stubs: FileEntity[] = contents
      .map(attachmentStubFromBlock)
      .filter((f): f is FileEntity => f !== null)
    if (stubs.length === 0) return

    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File
    if (!fileStore) return

    fileStore.restoreFilesFromEdit(stubs)
  },

  // Reactive companion to `beforeSendMessage` — drives ChatInput's
  // Send-button disable state. Subscribes to `uploadingFiles` via the
  // raw zustand hook so re-renders fire when upload status flips.
  useSendBlocker: () => {
    const uploadingFiles = useFileStore(s => s.uploadingFiles)
    const inFlight = Array.from(uploadingFiles.values()).some(
      f => f.status === 'pending' || f.status === 'uploading',
    )
    return inFlight ? { reason: 'uploading' } : null
  },

  // Click-time defensive cancel — in case the user clicks before the
  // disable lands (race) or some other extension's useSendBlocker
  // doesn't propagate. Same semantics as the useSendBlocker hook.
  beforeSendMessage: async () => {
    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File

    // Check if there are any files still uploading (use action method to avoid React hooks)
    if (fileStore.isUploading()) {
      console.log('[FileExtension] Blocking message send - files still uploading')

      return {
        cancel: true,
        errorMessage: 'Please wait for files to finish uploading',
      }
    }

    return { cancel: false }
  },

  // Compose request fields to add file_ids to send message request
  composeRequestFields: async () => {
    const { Stores } = await import('@ziee/framework/stores')

    // Call action method to get file IDs (actions don't trigger React hooks)
    const fileStore = Stores.File
    const fileIds = fileStore.getFileIds()

    console.log('[FileExtension] composeRequestFields - fileIds:', fileIds)

    if (fileIds.length === 0) {
      console.log('[FileExtension] No files selected')
      return {}
    }

    console.log('[FileExtension] ✅ Returning file_ids:', fileIds)
    return { file_ids: fileIds }
  },

  // Backup files before clearing (this runs after composeRequestFields)
  onMessageSent: async () => {
    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File

    // Backup files before clearing
    fileStore.setBackupFiles()
    fileStore.clearFiles()
    console.log('[FileExtension] Backed up and cleared files after message sent')
    return {}
  },

  // Restore files on stream error
  onStreamError: async (_error: Error) => {
    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File

    // Restore files from backup
    fileStore.restoreFromBackup()
    console.log('[FileExtension] Restored files from backup after stream error')
    return {}
  },

  // Clear backup on successful completion
  afterStreamComplete: async (_message) => {
    const { Stores } = await import('@ziee/framework/stores')
    const fileStore = Stores.File

    // Clear backup since message was sent successfully
    fileStore.clearBackup()
    console.log('[FileExtension] Cleared file backup after successful stream')
    return {}
  },

  // Register content type components
  contentTypes: {
    file_attachment: FileAttachmentRenderer,
    // Inline images (url / base64 / file source) — previously fell through to
    // the "Unknown content type: image" placeholder.
    image: ImageContent,
    // Tool-returned files (resource_links) render INLINE at the tool_result
    // block's position via this renderer — not aggregated into a footer. The
    // MCP extension intentionally does NOT register `tool_result` so this one
    // wins (the registry returns the first renderer for a content type).
    tool_result: MessageFilesView,
  },

  // Register slot components
  slots: {
    // File attach item in + dropdown
    toolbar_plus_items: { component: FileAttachMenuItem, order: 10 },
    // File preview list above textarea
    input_area_prefix: { component: FilePreviewList, order: 10 },
    // Composer-attached file listeners (both locate the composer via
    // [data-chat-composer]): the drag-and-drop overlay + the paste-image
    // handler (ITEM-8). Grouped into the single input_area_suffix entry since a
    // slot takes one component per extension.
    input_area_suffix: { component: ComposerFileListeners, order: 10 },
  },
})

export default fileExtension
