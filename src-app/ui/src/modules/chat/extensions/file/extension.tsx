import React from 'react'
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
