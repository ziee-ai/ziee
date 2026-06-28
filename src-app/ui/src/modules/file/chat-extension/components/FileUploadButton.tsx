import { Paperclip } from 'lucide-react'
import { Button, Tooltip, Upload, message } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

// Maximum file size (100MB)
const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * FileUploadButton Component
 * Toolbar button that triggers file picker for uploading files
 */
export function FileUploadButton() {
  // Access file extension store directly via Stores.Chat (reactive via store proxy)
  const { uploadFiles } = Stores.File
  const canUpload = usePermission(Permissions.FilesUpload)

  if (!canUpload) return null

  const handleFiles = (incoming: File[]) => {
    // Surface an error for any oversized file
    incoming
      .filter((f) => f.size > MAX_FILE_SIZE)
      .forEach((f) =>
        message.error(`File ${f.name} is too large. Maximum size is 100MB.`),
      )

    // Collect all valid files from the batch
    const files = incoming.filter((f) => f.size <= MAX_FILE_SIZE)

    if (files.length > 0) {
      // Upload files using store
      uploadFiles(files).catch((error: any) => {
        console.error('Upload failed:', error)
        message.error('Failed to upload files')
      })
    }
  }

  return (
    <Upload
      multiple
      accept="*/*"
      label="Attach files"
      onFiles={handleFiles}
      className="!border-0 !p-0 inline-flex"
    >
      <Tooltip title="Attach files">
        <Button
          variant="ghost"
          icon={<Paperclip />}
          aria-label="Attach files"
          data-testid="file-upload-button"
        />
      </Tooltip>
    </Upload>
  )
}
