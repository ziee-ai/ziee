import { Button, Upload, message } from 'antd'
import { PaperClipOutlined } from '@ant-design/icons'
import type { UploadProps } from 'antd'
import { Stores } from '@/core/stores'

// Maximum file size (100MB)
const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * FileUploadButton Component
 * Toolbar button that triggers file picker for uploading files
 */
export function FileUploadButton() {
  // Access file extension store directly via Stores.Chat (reactive via store proxy)
  const { uploadFiles } = Stores.Chat.FileStore

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

    // Prevent default upload behavior (we handle it ourselves)
    return false
  }

  return (
    <Upload
      multiple
      showUploadList={false}
      beforeUpload={handleBeforeUpload}
      accept="*/*"
    >
      <Button
        type="text"
        icon={<PaperClipOutlined />}
        title="Attach files"
        data-testid="file-upload-button"
      />
    </Upload>
  )
}
