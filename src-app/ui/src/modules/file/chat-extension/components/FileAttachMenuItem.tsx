import { Paperclip } from 'lucide-react'
import { Upload, message } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * FileAttachMenuItem Component
 * Menu item inside the + dropdown for attaching files
 */
export function FileAttachMenuItem() {
  const { uploadFiles } = Stores.File
  const { close } = usePlusDropdown()

  const handleFiles = (incoming: File[]) => {
    close()
    incoming
      .filter(f => f.size > MAX_FILE_SIZE)
      .forEach(f =>
        message.error(`File ${f.name} is too large. Maximum size is 100MB.`),
      )
    const files = incoming.filter(f => f.size <= MAX_FILE_SIZE)
    if (files.length > 0) {
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
      label="Attach files or photos"
      onFiles={handleFiles}
      data-testid="file-attach-menu-upload"
      className="!flex-row !items-center !justify-start !border-0 !p-0 gap-2 px-3 py-2 rounded-md text-foreground min-w-[180px] hover:bg-muted"
    >
      <Paperclip className="size-4" />
      <span className="text-sm">Attach files or photos</span>
    </Upload>
  )
}
