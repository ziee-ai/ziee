import { Paperclip } from 'lucide-react'
import { Button, Tooltip, Upload, message } from '@ziee/kit'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { composerPaneKey } from '@/modules/file/stores/file'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import {
  MAX_FILE_UPLOAD_BYTES as MAX_FILE_SIZE,
  MAX_FILE_UPLOAD_LABEL,
} from '@/modules/file/constants'
import { File as FileStore } from '@/modules/file/stores/file'

/**
 * FileUploadButton Component
 * Toolbar button that triggers file picker for uploading files
 */
export function FileUploadButton() {
  // Access file extension store directly via Chat (reactive via store proxy)
  const { uploadFiles } = FileStore
  const paneKey = composerPaneKey(useChatPaneOrNull()?.paneId)
  const canUpload = usePermission(Permissions.FilesUpload)

  if (!canUpload) return null

  const handleFiles = (incoming: File[]) => {
    // Surface an error for any oversized file
    incoming
      .filter((f) => f.size > MAX_FILE_SIZE)
      .forEach((f) =>
        message.error(
          `File ${f.name} is too large. Maximum size is ${MAX_FILE_UPLOAD_LABEL}.`,
        ),
      )

    // Collect all valid files from the batch
    const files = incoming.filter((f) => f.size <= MAX_FILE_SIZE)

    if (files.length > 0) {
      // Upload files using store
      uploadFiles(paneKey, files).catch((error: any) => {
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
      data-testid="file-upload-button-area"
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
