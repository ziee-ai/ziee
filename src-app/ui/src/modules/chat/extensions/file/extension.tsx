import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'
import { createFileExtensionStore } from '@/modules/chat/extensions/file/File.store'
import { FilePreviewList } from '@/modules/chat/extensions/file/components/FilePreviewList'
import { FileAttachMenuItem } from '@/modules/chat/extensions/file/components/FileAttachMenuItem'
import { FileCard } from '@/modules/chat/extensions/file/components/FileCard'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { File as FileEntity, MessageContent, MessageContentDataFileAttachment } from '@/api-client/types'

// Augment the central PanelRendererMap so `displayInRightPanel({ type: 'file',
// data: ... })` and `registerPanelRenderer('file', ...)` are type-checked.
declare module '@/modules/chat/core/stores/Chat.store' {
  interface PanelRendererMap {
    file: { fileId: string }
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

  // Fallback entity from content block data (shown while store fetches the full entity)
  const fallback = {
    id: fileData.file_id,
    filename: fileData.filename,
    file_size: fileData.file_size,
    mime_type: fileData.mime_type ?? undefined,
    has_thumbnail: false,
    preview_page_count: 0,
    created_at: '',
    updated_at: '',
    user_id: '',
    created_by: '',
    processing_metadata: null,
    text_page_count: 0,
  }

  // Reactive subscription to messageFilesCache — re-renders when the file entity is loaded
  const messageFilesCache = Stores.Chat.FileStore.messageFilesCache
  const file = messageFilesCache.get(fileData.file_id) ?? fallback

  // Trigger background load on first access (deferred inside store action — safe in render)
  Stores.Chat.FileStore.getMessageFile(fileData.file_id, fallback)

  return (
    <FileCard
      file={file}
      variant={isUser ? 'square' : 'row'}
      showFileName={true}
      canRemove={false}
      canDelete={false}
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

  // Register store for file upload state
  store: {
    name: 'FileStore',
    createStore: createFileExtensionStore,
  },

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
    const { FilePanel: FilePanelComponent } = await import('./components/FilePanel')
    const { FileOutlined: FileOutlinedIcon } = await import('@ant-design/icons')
    const { Spin: SpinComponent } = await import('antd')
    const { Stores: StoresRef } = await import('@/core/stores')

    registerPanelRenderer('file', {
      icon: <FileOutlinedIcon />,
      component: ({ fileId }) => {
        const { selectedFiles, messageFilesCache } = StoresRef.Chat.FileStore
        const file = selectedFiles.get(fileId) ?? messageFilesCache.get(fileId) ?? null
        if (!file) return <SpinComponent />
        return <FilePanelComponent file={file} />
      },
    })

    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@/core/stores')

    useChatStore.subscribe(
      state => state.editingMessage,
      async (editingMessage) => {
        const fileStore = Stores.Chat.__state.FileStore
        if (!fileStore) return

        if (editingMessage) {
          // Restore file_attachment content blocks from the edited message
          const fileContents = editingMessage.contents.filter(
            c => c.content_type === 'file_attachment'
          )
          if (fileContents.length > 0) {
            // Phase 1 — Synchronous: build stubs from content block data immediately.
            // This ensures selectedFiles is populated before the user can click Send.
            const stubs: FileEntity[] = fileContents.map(c => {
              const data = c.content as MessageContentDataFileAttachment
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
              }
            })
            fileStore.restoreFilesFromEdit(stubs)

            // Phase 2 — Async: upgrade stubs with full server entities (enables thumbnails).
            try {
              const fullFiles = await Promise.all(
                fileContents.map(c => {
                  const data = c.content as MessageContentDataFileAttachment
                  return ApiClient.File.get({ file_id: data.file_id })
                })
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
   * Provide file attachment content blocks for the temp user message created
   * during sendMessage(). Without this, the message bubble shows no file
   * previews until loadMessages() replaces the temp message with the real one.
   */
  provideUserContent: async (_text: string, _composedRequest: any): Promise<MessageContent[]> => {
    const { Stores } = await import('@/core/stores')
    const fileStore = Stores.Chat.__state.FileStore
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
      } as MessageContentDataFileAttachment,
      sequence_order: index + 1, // text block is at sequence_order 0
      created_at: now,
      updated_at: now,
    }))
  },

  // Check if files are still uploading before sending message
  beforeSendMessage: async () => {
    const { Stores } = await import('@/core/stores')
    const fileStore = Stores.Chat.__state.FileStore

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
    const { Stores } = await import('@/core/stores')

    // Call action method to get file IDs (actions don't trigger React hooks)
    const fileStore = Stores.Chat.__state.FileStore
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
    const { Stores } = await import('@/core/stores')
    const fileStore = Stores.Chat.__state.FileStore

    // Backup files before clearing
    fileStore.setBackupFiles()
    fileStore.clearFiles()
    console.log('[FileExtension] Backed up and cleared files after message sent')
    return {}
  },

  // Restore files on stream error
  onStreamError: async (_error: Error) => {
    const { Stores } = await import('@/core/stores')
    const fileStore = Stores.Chat.__state.FileStore

    // Restore files from backup
    fileStore.restoreFromBackup()
    console.log('[FileExtension] Restored files from backup after stream error')
    return {}
  },

  // Clear backup on successful completion
  afterStreamComplete: async (_message) => {
    const { Stores } = await import('@/core/stores')
    const fileStore = Stores.Chat.__state.FileStore

    // Clear backup since message was sent successfully
    fileStore.clearBackup()
    console.log('[FileExtension] Cleared file backup after successful stream')
    return {}
  },

  // Register content type components
  contentTypes: {
    file_attachment: FileAttachmentRenderer,
  },

  // Register slot components
  slots: {
    // File attach item in + dropdown
    toolbar_plus_items: { component: FileAttachMenuItem, order: 10 },
    // File preview list above textarea
    input_area_prefix: { component: FilePreviewList, order: 10 },
  },
})

export default fileExtension
