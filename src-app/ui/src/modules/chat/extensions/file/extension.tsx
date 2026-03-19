import React from 'react'
import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'
import { createFileExtensionStore } from '@/modules/chat/extensions/file/File.store'
import { FilePreviewList } from '@/modules/chat/extensions/file/components/FilePreviewList'
import { FileAttachMenuItem } from '@/modules/chat/extensions/file/components/FileAttachMenuItem'
import { FileCard } from '@/modules/chat/extensions/file/components/FileCard'
import { ApiClient } from '@/api-client'
import type { File as FileEntity, MessageContentDataFileAttachment } from '@/api-client/types'

/**
 * File attachment content renderer component
 * Renders file attachments in message bubbles using FileCard
 */
function FileAttachmentRenderer({ content: data }: ContentRendererProps) {
  const [file, setFile] = React.useState<FileEntity | null>(null)
  const [loading, setLoading] = React.useState(true)

  // data is the full MessageContent object, data.content has the file attachment data
  const fileData = data.content as MessageContentDataFileAttachment

  // Fetch full file info using file_id
  React.useEffect(() => {
    if (!fileData?.file_id) {
      setLoading(false)
      return
    }

    const fetchFileInfo = async () => {
      try {
        const fileInfo = await ApiClient.File.get({ file_id: fileData.file_id })
        setFile(fileInfo)
      } catch (error) {
        console.error('Failed to fetch file info:', error)
      } finally {
        setLoading(false)
      }
    }

    fetchFileInfo()
  }, [fileData?.file_id])

  if (!fileData?.file_id || !fileData?.filename) {
    return null
  }

  if (loading) {
    return (
      <div className="inline-block">
        <div className="min-h-20 min-w-20">Loading...</div>
      </div>
    )
  }

  if (!file) {
    return null
  }

  return (
    <div className="inline-block">
      <FileCard
        file={file}
        showFileName={true}
        canRemove={false}
        canDelete={false}
      />
    </div>
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
    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@/core/stores')

    useChatStore.subscribe(
      state => state.editingMessage,
      (editingMessage) => {
        const fileStore = Stores.Chat.__state.FileStore
        if (!fileStore) return

        if (editingMessage) {
          // Restore file_attachment content blocks from the edited message
          const fileContents = editingMessage.contents.filter(
            c => c.content_type === 'file_attachment'
          )
          if (fileContents.length > 0) {
            const files = fileContents.map(c => (c.content as any).file as FileEntity)
            fileStore.restoreFilesFromEdit(files)
          }
        } else {
          // Edit ended (cancel or send) — clear the file selection
          fileStore.clearFiles()
        }
      }
    )
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
