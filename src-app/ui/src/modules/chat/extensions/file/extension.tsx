import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '../../core/extensions'
import { createFileExtensionStore } from './File.store'
import { FilePreviewList } from './components/FilePreviewList'
import { FileUploadButton } from './components/FileUploadButton'
import { FileCard } from './components/FileCard'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'

/**
 * File attachment data structure (from backend MessageContentData::Extension)
 */
interface FileAttachment {
  id: string
  name: string
  size: number
  mime_type: string
}

/**
 * File attachment content renderer component
 * Renders file attachments in message bubbles using FileCard
 */
function FileAttachmentRenderer({ content }: ContentRendererProps) {
  const fileData = content.content as FileAttachment

  if (!fileData?.id || !fileData?.name) {
    return null
  }

  // Map FileAttachment to File entity structure for FileCard
  const file: FileEntity = {
    id: fileData.id,
    filename: fileData.name,
    file_size: fileData.size,
    mime_type: fileData.mime_type || 'application/octet-stream',
    user_id: '', // Not needed for display
    created_at: '', // Not needed for display
    updated_at: '', // Not needed for display
    has_thumbnail: false,
    preview_page_count: 0,
    text_page_count: 0,
    processing_metadata: null,
  }

  // Download handler using API client
  const handleClick = async () => {
    try {
      // Use API client to download file (handles authentication)
      const response = await ApiClient.File.download({ file_id: fileData.id })

      // Create download link
      const blob = response instanceof Blob ? response : new Blob([response])
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = fileData.name
      document.body.appendChild(a)
      a.click()
      window.URL.revokeObjectURL(url)
      document.body.removeChild(a)
    } catch (error) {
      console.error('Failed to download file:', error)
    }
  }

  return (
    <div className="inline-block">
      <FileCard
        file={file}
        showFileName={true}
        canRemove={false}
        canDelete={false}
        onClick={handleClick}
      />
    </div>
  )
}

/**
 * File Extension
 * Handles file attachment upload and rendering in messages
 */
const fileExtension: ChatExtension = createExtension({
  name: 'FileStore',
  description: 'Handles file attachment upload and rendering',
  priority: 80,

  // Register store for file upload state
  createStore: createFileExtensionStore,

  // Check if files are still uploading before sending message
  beforeSendMessage: async (_message: string) => {
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

    return {}
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

  // Clear files after message is sent (this runs after composeRequestFields)
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    Stores.Chat.__state.FileStore.clearFiles()
    console.log('[FileExtension] Cleared files after message sent')
    return {}
  },

  // Register content type components
  contentTypes: {
    file_attachment: FileAttachmentRenderer,
  },

  // Register slot components
  slots: {
    // File upload button in toolbar
    toolbar_actions: { component: FileUploadButton, order: 10 },
    // File preview list above textarea
    input_area_prefix: { component: FilePreviewList, order: 10 },
  },
})

export default fileExtension
