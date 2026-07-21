import { Alert, ScrollArea } from '@ziee/kit'
import { FileCard } from '@/modules/file/components/FileCard'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { composerPaneKey } from '@/modules/file/stores/file'
import type { FileUploadProgress } from '@/modules/file/stores/file'
import type { File as FileEntity } from '@/api-client/types'
import { File as FileStore } from '@/modules/file/stores/file'
import { Chat as ChatStore } from '@/modules/chat/core/stores/chatBridge'

/**
 * FilePreviewList Component
 * Displays horizontal scrollable list of selected/uploading files
 * Used in ChatInput to show files before sending
 * Matches reference implementation styling
 */
export function FilePreviewList() {
  const pane = useChatPaneOrNull()
  // Open into THIS pane's right panel (ITEM-36), not the focused pane's.
  const chat = (pane?.store ?? ChatStore) as typeof ChatStore
  // THIS pane's composer buffer key (ITEM-32): show only this pane's files.
  const paneKey = composerPaneKey(pane?.paneId)
  const {
    selectedFiles,
    uploadingFiles,
    fileOwner,
    uploadOwner,
    removeFile,
    removeUploadingFile,
    retryUpload,
  } = FileStore

  // Filter the shared buffers to this pane's owned entries.
  const paneSelected = Array.from(
    selectedFiles.entries() as IterableIterator<[string, FileEntity]>,
  )
    .filter(([id]) => composerPaneKey(fileOwner.get(id)) === paneKey)
    .map(([, f]) => f)
  const paneUploading = Array.from(
    uploadingFiles.entries() as IterableIterator<[string, FileUploadProgress]>,
  )
    .filter(([id]) => composerPaneKey(uploadOwner.get(id)) === paneKey)
    .map(([, p]) => p)

  const hasFiles = paneSelected.length > 0 || paneUploading.length > 0

  if (!hasFiles) {
    return null
  }

  // Upload-time suitability advisory (Track A §2b): non-blocking nudge when a
  // file type reads poorly (e.g. PowerPoint, scanned PDF, archive). The backend
  // annotates `processing_metadata.suitability/suggestion` at upload time.
  const advisories = paneSelected
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
          {paneUploading.map((progress) => (
            <div key={progress.id} className="flex-1 min-w-20 max-w-24">
              <FileCard
                uploadProgress={progress}
                onRemove={() => removeUploadingFile(progress.id)}
                onRetry={() => retryUpload(paneKey, progress.id)}
                variant="square"
              />
            </div>
          ))}

          {/* Selected files (upload completed) */}
          {paneSelected.map((file) => (
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
                  // displayInRightPanel is an action — callable directly from
                  // an event handler (actions are hook-free).
                  chat.displayInRightPanel({
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
