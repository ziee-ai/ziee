import { Paperclip } from 'lucide-react'
import { Upload, message } from '@/components/ui'
import { Stores } from '@/core/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { composerPaneKey } from '@/modules/file/stores/File.store'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import {
  MAX_FILE_UPLOAD_BYTES as MAX_FILE_SIZE,
  MAX_FILE_UPLOAD_LABEL,
} from '@/modules/file/constants'

/**
 * FileAttachMenuItem Component
 * Menu item inside the + dropdown for attaching files
 */
export function FileAttachMenuItem() {
  const { uploadFiles } = Stores.File
  const paneKey = composerPaneKey(useChatPaneOrNull()?.paneId)
  const { close } = usePlusDropdown()
  // Gate on files::upload (mirrors FilePasteHandler / FileUploadArea). Without
  // it, a user lacking the grant saw the "Attach files or photos" + menu item
  // and could trigger an upload that the backend 403s.
  const canUpload = usePermission(Permissions.FilesUpload)
  if (!canUpload) return null

  const handleFiles = (incoming: File[]) => {
    close()
    incoming
      .filter(f => f.size > MAX_FILE_SIZE)
      .forEach(f =>
        message.error(
          `File ${f.name} is too large. Maximum size is ${MAX_FILE_UPLOAD_LABEL}.`,
        ),
      )
    const files = incoming.filter(f => f.size <= MAX_FILE_SIZE)
    if (files.length > 0) {
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
      label="Attach files or photos"
      onFiles={handleFiles}
      data-testid="file-attach-menu-upload"
      className="!flex-row !items-center !justify-start !border-0 !px-3 !py-1.5 gap-2 rounded-md text-foreground hover:bg-muted whitespace-nowrap !text-start"
    >
      {/* Match PlusMenuItem exactly: icon wrapped so it's vertically centered
          and sized consistently; label left-aligned (!text-start above cancels
          the Upload dropzone's default `text-center`, which otherwise centers
          this label inside its flex-1 span and misaligns it vs the other +
          menu items). */}
      <span className="shrink-0 inline-flex items-center [&_svg]:size-4">
        <Paperclip />
      </span>
      <span className="min-w-0 flex-1 truncate text-sm">Attach files or photos</span>
    </Upload>
  )
}
