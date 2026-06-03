import { useEffect } from 'react'
import { Upload, message, theme } from 'antd'
import type { UploadProps } from 'antd'
import { Stores } from '@/core/stores'

const { Dragger } = Upload

// Maximum file size (100MB)
const MAX_FILE_SIZE = 100 * 1024 * 1024

export interface FileUploadAreaProps {
  children: React.ReactNode
}

/**
 * FileUploadArea Component
 * Drag-and-drop overlay for file uploads
 * Wraps the chat input area to accept dropped files
 */
export function FileUploadArea({ children }: FileUploadAreaProps) {
  const { token } = theme.useToken()

  // Inject styles using theme tokens
  useEffect(() => {
    const styleId = 'file-upload-area-styles'

    // Remove existing style if present
    const existingStyle = document.getElementById(styleId)
    if (existingStyle) {
      existingStyle.remove()
    }

    // Create and inject new styles with theme tokens
    const style = document.createElement('style')
    style.id = styleId
    style.textContent = `
      .file-upload-dragger .ant-upload {
        padding: 0 !important;
        background: none !important;
        border: none !important;
      }

      .file-upload-dragger .ant-upload-drag {
        background: none !important;
        border: none !important;
      }

      .file-upload-dragger .ant-upload-drag-container {
        display: none;
      }

      .file-upload-dragger.ant-upload-drag:not(.ant-upload-disabled):hover {
        border-color: transparent !important;
      }

      /* Show overlay when dragging - using theme colors */
      .file-upload-dragger.ant-upload-drag.ant-upload-drag-hover {
        background: ${token.colorPrimaryBg} !important;
        border: 2px dashed ${token.colorPrimary} !important;
      }

      .file-upload-dragger.ant-upload-drag.ant-upload-drag-hover .ant-upload-drag-container {
        display: flex !important;
        align-items: center;
        justify-content: center;
        min-height: 200px;
      }
    `
    document.head.appendChild(style)

    return () => {
      const style = document.getElementById(styleId)
      if (style) {
        style.remove()
      }
    }
  }, [token.colorPrimaryBg, token.colorPrimary])

  // Access file extension store directly via Stores.Chat (reactive via store proxy)
  const { uploadFiles } = Stores.File

  const handleBeforeUpload: UploadProps['beforeUpload'] = (file, fileList) => {
    // Validate file size
    if (file.size > MAX_FILE_SIZE) {
      message.error(`File ${file.name} is too large. Maximum size is 100MB.`)
      return Upload.LIST_IGNORE
    }

    // Only upload on the last file to avoid duplicates
    // beforeUpload is called once for each file, so we need to wait for the last one
    const isLastFile = fileList[fileList.length - 1] === file

    if (isLastFile) {
      // Collect all valid files from the batch
      const files = fileList.filter((f) => f.size <= MAX_FILE_SIZE)

      if (files.length > 0) {
        // Upload files using store
        uploadFiles(files as File[])
          .catch((error: any) => {
            console.error('Upload failed:', error)
            message.error('Failed to upload files')
          })
      }
    }

    // Prevent default upload behavior
    return false
  }

  return (
    <Dragger
      multiple
      showUploadList={false}
      beforeUpload={handleBeforeUpload}
      accept="*/*"
      openFileDialogOnClick={false} // Only handle drag-and-drop, not clicks
      className="file-upload-dragger"
      style={{
        background: 'none',
        border: 'none',
        padding: 0,
      }}
    >
      {children}
    </Dragger>
  )
}
