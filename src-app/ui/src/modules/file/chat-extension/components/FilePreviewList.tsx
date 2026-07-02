import { Alert, ScrollArea } from '@/components/ui'
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
  const {
    selectedFiles,
    uploadingFiles,
    removeFile,
    removeUploadingFile,
    retryUpload,
  } = Stores.File

  const hasFiles = selectedFiles.size > 0 || uploadingFiles.size > 0

  if (!hasFiles) {
    return null
  }

  // Upload-time suitability advisory (Track A §2b): non-blocking nudge when a
  // file type reads poorly (e.g. PowerPoint, scanned PDF, archive). The backend
  // annotates `processing_metadata.suitability/suggestion` at upload time.
  const advisories = Array.from(
    selectedFiles.values() as IterableIterator<FileEntity>,
  )
    .map(f => ({
      f,
      meta: (f.processing_metadata ?? {}) as {
        suitability?: string
        suggestion?: string
      },
    }))
    .filter(x => x.meta.suitability === 'low' && !!x.meta.suggestion)

  return (
    <>
      <div className="mb-1.5">
        {/* Horizontal overflow scrolls via the app's overlay ScrollArea (not a
            native scrollbar). px-1 py-1 gutter keeps a focused card's ring from
            being clipped inside the scroll viewport. */}
        <ScrollArea axis="x" className="w-full">
        <div
          className="flex gap-2 w-max px-1 py-1"
          role="list"
          aria-label="Attached files"
        >
          {/* Uploading files */}
          {Array.from(uploadingFiles.values() as IterableIterator<FileUploadProgress>).map((progress) => (
            <div key={progress.id} className="flex-1 min-w-20 max-w-24">
              <FileCard
                uploadProgress={progress}
                onRemove={() => removeUploadingFile(progress.id)}
                onRetry={() => retryUpload(progress.id)}
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
                  // `__state` (not the render-only proxy) for store access from
                  // an event handler — the proxy fires React hooks on access.
                  Stores.Chat.__state.displayInRightPanel({
                    id: file.id,
                    title: file.filename,
                    type: 'file',
                    // Open at the file's current head version — the composer
                    // always holds the head entity (version == current_version_id).
                    data: { fileId: file.id, version: file.version },
                  })
                }
              />
            </div>
          ))}
        </div>
        </ScrollArea>

        {advisories.length > 0 && (
          <div className="flex flex-col gap-1 mt-2">
            {advisories.map(({ f, meta }) => (
              <Alert
                key={f.id}
                data-testid={`file-preview-advisory-${f.id}`}
                tone="warning"
                title={
                  <span>
                    <strong>{f.filename}</strong>: {meta.suggestion}
                  </span>
                }
              />
            ))}
          </div>
        )}
      </div>
    </>
  )
}
