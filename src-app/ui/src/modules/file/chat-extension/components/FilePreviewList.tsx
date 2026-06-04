import { Divider } from 'antd'
import { FileCard } from '@/modules/file/components/FileCard'
import { Stores } from '@/core/stores'
import type { FileUploadProgress } from '@/modules/file/stores/File.store'
import type { File as FileEntity } from '@/api-client/types'

/**
 * FilePreviewList Component
 * Displays horizontal scrollable list of selected/uploading files
 * Used in ChatInput to show files before sending
 * Matches reference implementation styling
 */
export function FilePreviewList() {
  // Access file extension store directly via Stores.Chat (reactive via store proxy)
  const { selectedFiles, uploadingFiles, removeFile, removeUploadingFile } =
    Stores.File

  const hasFiles = selectedFiles.size > 0 || uploadingFiles.size > 0

  if (!hasFiles) {
    return null
  }

  return (
    <>
      <Divider style={{ margin: 0 }} />
      <div style={{ padding: '8px' }}>
        <div className="flex gap-2 w-full overflow-x-auto">
          {/* Uploading files */}
          {Array.from(uploadingFiles.values() as IterableIterator<FileUploadProgress>).map((progress) => (
            <div key={progress.id} className="flex-1 min-w-20 max-w-24">
              <FileCard
                uploadProgress={progress}
                onRemove={() => removeUploadingFile(progress.id)}
                variant="square"
              />
            </div>
          ))}

          {/* Selected files (upload completed) */}
          {Array.from(selectedFiles.values() as IterableIterator<FileEntity>).map((file) => (
            <div key={file.id} className="flex-1 min-w-20 max-w-24">
              <FileCard
                file={file}
                canDelete={false}
                canRemove={true}
                onRemove={() => removeFile(file.id)}
                variant="square"
                // Chat composer is a chat surface → side-by-side
                // right panel beats the global drawer for the
                // "review while chatting" flow.
                onClick={() =>
                  Stores.Chat.displayInRightPanel({
                    id: file.id,
                    title: file.filename,
                    type: 'file',
                    data: { fileId: file.id },
                  })
                }
              />
            </div>
          ))}
        </div>
      </div>
    </>
  )
}
