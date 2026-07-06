import { useState } from 'react'
import { X, Trash2, FileText, Download, RotateCw } from 'lucide-react'
import {
  Button, Checkbox, Progress, Spin, Tooltip, Text, message as kitMessage,
  Attachment, AttachmentMedia, AttachmentContent, AttachmentTitle,
  AttachmentDescription, AttachmentActions, AttachmentTrigger,
} from '@/components/ui'
import { Confirm } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type File as FileEntity } from '@/api-client/types'
import type { FileUploadProgress } from '@/modules/file/stores/File.store'
import { getViewer } from '@/modules/file/registry/fileViewerRegistry'

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

// Icon comes from the matching viewer module's entry. Falls back to a generic
// file-text icon when no viewer is registered or the viewer omitted the field.
function getFileIcon(file: FileEntity): React.ReactNode {
  return getViewer(file.filename, file.mime_type ?? undefined)?.icon
    ?? <FileText />
}

export interface FileCardProps {
  file?: FileEntity
  uploadProgress?: FileUploadProgress
  onRemove?: () => void
  onClick?: () => void
  showFileName?: boolean
  canRemove?: boolean
  canDelete?: boolean
  variant?: 'row' | 'square'
  /** Square only — when true, drops the hard-coded 96px width on the
   *  card wrapper so the parent layout controls width (e.g. a CSS
   *  Grid track with auto-fill minmax). The aspect-ratio enforcer
   *  keeps the card square at whatever width the grid hands it.
   *  Default false to preserve the legacy fixed-96px sizing the chat
   *  composer's FilePreviewList depends on. */
  stretch?: boolean
  /** Row only — appended to the trailing edge in place of the default
   *  Download button. Use this slot to pass a Confirm-wrapped
   *  delete button, retry button, etc. */
  actions?: React.ReactNode
  /** Overrides the default "{label} · {ext}" subtitle line (row only).
   *  Callers can pass the size, attached-at date, etc. */
  subtitle?: React.ReactNode
  /** Row only — when true, prepends a hover-revealed Checkbox cell. */
  selectable?: boolean
  selected?: boolean
  onSelectChange?: (selected: boolean) => void
  /** Surfaced on the upload-error row variant — when provided, the
   *  trailing cancel button is replaced with a retry button. */
  onRetry?: () => void
}

/**
 * FileCard Component
 *
 * Built on the kit `Attachment` primitive (media / content / actions /
 * full-card trigger). Two variants:
 * - 'row' (default): horizontal card for assistant artifacts + knowledge lists
 * - 'square': vertical card for user message attachments and the composer input
 */
export function FileCard({
  file,
  uploadProgress,
  onRemove,
  onClick,
  showFileName = true,
  canRemove = true,
  canDelete = false,
  variant = 'row',
  stretch = false,
  actions,
  subtitle,
  selectable = false,
  selected = false,
  onSelectChange,
  onRetry,
}: FileCardProps) {
  const canDownload = usePermission(Permissions.FilesDownload)
  // Controlled remove-confirm (square variant). Decoupled from the button so the
  // button is a plain tooltip'd Button, not a Confirm trigger — a Tooltip + an
  // AlertDialog trigger on the SAME node fight over hover state and flicker.
  const [removeOpen, setRemoveOpen] = useState(false)

  // Reactive subscription: re-render when the thumbnail blob URL lands.
  const thumbnailUrls = Stores.File.thumbnailUrls
  const thumbnailUrl = file ? (thumbnailUrls.get(file.id) ?? null) : null
  // Trigger the thumbnail load on first render when this file has one
  // (idempotent — guarded by thumbnailLoadingSet in the store). loadMessageFile
  // no longer eager-loads thumbnails, so each displaying component owns its own
  // load — mirrors ImageBody.
  if (file?.has_thumbnail && file.preview_page_count > 0 && !thumbnailUrl) {
    Stores.File.getThumbnailUrl(file.id, file)
  }

  // Trigger lazy load on cache miss. The action is deferred inside the
  // store (safe in render — same pattern as FileAttachmentRenderer's
  // `getMessageFile` call in chat-extension/extension.tsx). Internally
  // guarded by `has_thumbnail && preview_page_count > 0` so non-image
  // files don't trigger a wasted fetch.
  if (file && !thumbnailUrl) {
    Stores.File.getThumbnailUrl(file.id, file)
  }

  const handleCardClick = () => {
    if (!file || uploadProgress) return
    if (onClick) { onClick(); return }
    // Default: open the global file-preview drawer. Chat surfaces override
    // `onClick` to open the side-by-side right-panel instead.
    Stores.FilePreviewDrawer.openPreview(file)
  }

  // ── Row upload-progress ────────────────────────────────────────────────────
  if (uploadProgress && variant === 'row') {
    const isError = uploadProgress.status === 'error'
    const percent = Math.round(uploadProgress.progress)
    const ext =
      (uploadProgress.filename?.split('.').pop()?.toUpperCase()) || 'FILE'
    return (
      <Attachment
        orientation="horizontal"
        state={isError ? 'error' : 'uploading'}
        className="w-full"
        data-testid="file-card-uploading"
        data-filename={uploadProgress.filename}
      >
        <AttachmentMedia>
          {isError ? (
            <Text className="!text-[9px] rounded px-1 text-white bg-destructive">
              ERROR
            </Text>
          ) : (
            <Progress
              shape="circle"
              value={percent}
              size="sm"
              format={() => ext}
              aria-label={`Upload progress for ${uploadProgress.filename}`}
              data-testid="file-card-upload-progress"
            />
          )}
        </AttachmentMedia>
        <AttachmentContent>
          <AttachmentTitle title={uploadProgress.filename}>
            {uploadProgress.filename}
          </AttachmentTitle>
          <AttachmentDescription>
            {isError
              ? ((uploadProgress as { error?: string }).error ?? 'Upload failed')
              : `Uploading… ${percent}%`}
          </AttachmentDescription>
        </AttachmentContent>
        {isError && onRetry ? (
          <AttachmentActions>
            <Tooltip content="Retry upload">
              <Button
                variant="ghost"
                icon={<RotateCw />}
                onClick={() => onRetry()}
                aria-label={`Retry upload ${uploadProgress.filename}`}
                data-testid="file-card-retry-btn"
              />
            </Tooltip>
          </AttachmentActions>
        ) : (
          onRemove && (
            <AttachmentActions>
              <Tooltip content="Dismiss">
                <Button
                  variant="ghost"
                  icon={<X />}
                  onClick={() => onRemove()}
                  aria-label={`Dismiss ${uploadProgress.filename}`}
                  data-testid="file-card-dismiss-btn"
                />
              </Tooltip>
            </AttachmentActions>
          )
        )}
      </Attachment>
    )
  }

  // ── Square upload-progress ─────────────────────────────────────────────────
  if (uploadProgress) {
    const isError = uploadProgress.status === 'error'
    const ext = (uploadProgress.filename?.split('.').pop()?.toUpperCase()) || 'FILE'
    return (
      <Attachment
        orientation="vertical"
        state={isError ? 'error' : 'uploading'}
        className={stretch ? 'w-full' : 'w-24'}
        data-testid="file-card-uploading"
        data-filename={uploadProgress.filename}
      >
        <AttachmentMedia>
          {isError ? (
            onRetry ? (
              <Tooltip content="Retry upload">
                <Button
                  variant="ghost"
                  size="default"
                  icon={<RotateCw />}
                  onClick={() => onRetry()}
                  aria-label={`Retry upload ${uploadProgress.filename}`}
                  data-testid="file-card-square-retry-btn"
                />
              </Tooltip>
            ) : (
              <Text strong className="!text-[9px]">{ext}</Text>
            )
          ) : (
            <Spin label="Uploading" />
          )}
        </AttachmentMedia>
        {showFileName && (
          <AttachmentContent className="!px-2 !pb-1">
            <AttachmentTitle title={uploadProgress.filename}>
              {uploadProgress.filename}
            </AttachmentTitle>
            <AttachmentDescription>
              {formatFileSize(uploadProgress.size)}
            </AttachmentDescription>
          </AttachmentContent>
        )}
        {onRemove && (
          <AttachmentActions>
            <Tooltip content={isError ? 'Dismiss' : 'Cancel upload'}>
              <Button
                variant="ghost"
                size="default"
                icon={<X />}
                onClick={() => onRemove()}
                aria-label={
                  isError ? `Dismiss ${uploadProgress.filename}` : 'Cancel upload'
                }
                data-testid="file-card-cancel-btn"
              />
            </Tooltip>
          </AttachmentActions>
        )}
      </Attachment>
    )
  }

  if (!file) return null

  const ext = (file.filename?.split('.').pop()?.toUpperCase()) || 'FILE'
  const viewerLabel = getViewer(file.filename, file.mime_type ?? undefined)?.label ?? ext
  // Show whatever thumbnail the backend generated (images, PDFs, docs,
  // spreadsheets all get one), not just image mime types.
  const hasImage = !!thumbnailUrl

  // ── Row variant (assistant artifacts + knowledge management) ───────────────
  if (variant === 'row') {
    return (
      <Attachment
        orientation="horizontal"
        state="done"
        className="w-full cursor-pointer"
        data-testid="file-card"
        data-file-id={file.id}
        data-filename={file.filename}
      >
        {/* Full-card click target (native button → keyboard accessible). Sits
            UNDER the actions/checkbox (kit layers those at z-20). */}
        <AttachmentTrigger
          aria-label={`Open ${file.filename}`}
          onClick={handleCardClick}
        />

        {/* Optional multi-select checkbox — layered above the trigger. */}
        {selectable && (
          <div className="relative z-20 flex-shrink-0">
            <Checkbox
              checked={selected}
              onChange={checked => onSelectChange?.(checked)}
              aria-label={`Select ${file.filename}`}
              data-testid="file-card-select-checkbox"
            />
          </div>
        )}

        <AttachmentMedia variant={hasImage ? 'image' : 'icon'}>
          {hasImage ? (
            <img src={thumbnailUrl!} alt={file.filename} />
          ) : (
            getFileIcon(file)
          )}
        </AttachmentMedia>

        <AttachmentContent>
          <AttachmentTitle title={file.filename}>{file.filename}</AttachmentTitle>
          <AttachmentDescription>
            {subtitle ?? <>{viewerLabel} · {ext}</>}
          </AttachmentDescription>
        </AttachmentContent>

        {/* Trailing: caller-provided actions OR default Download button. */}
        {actions !== undefined ? (
          <AttachmentActions>{actions}</AttachmentActions>
        ) : (
          canDownload && (
            <AttachmentActions>
              <Tooltip content="Download">
                <Button
                  variant="ghost"
                  icon={<Download />}
                  aria-label={`Download ${file.filename}`}
                  data-testid="file-card-download-btn"
                  onClick={() => {
                    Stores.File.downloadFile(file)
                      .catch(() => kitMessage.error('Failed to download file'))
                  }}
                />
              </Tooltip>
            </AttachmentActions>
          )
        )}
      </Attachment>
    )
  }

  // ── Square variant (user message attachments & input area) ─────────────────
  // Wrapped in a Tooltip anchored to the OUTER span (a distinct node from the
  // inner AttachmentTrigger button) so hovering the card shows the full filename
  // without the trigger + tooltip fighting over hover state.
  return (
    <Tooltip content={file.filename}>
      <span className="block w-full">
        <Attachment
          orientation="vertical"
          state="done"
          // !p-0 + overflow-hidden = the preview thumbnail is flush to the card
          // border (no gutter); the card's rounded corners clip the image.
          className={`cursor-pointer overflow-hidden !p-0 ${stretch ? 'w-full' : 'w-24'}`}
          data-testid="file-card"
          data-file-id={file.id}
          data-filename={file.filename}
        >
          <AttachmentTrigger
            aria-label={`Open ${file.filename}`}
            onClick={handleCardClick}
          />

          <AttachmentMedia variant={hasImage ? 'image' : 'icon'} className="w-full rounded-none">
            {hasImage ? (
              <img src={thumbnailUrl!} alt={file.filename} />
            ) : (
              getFileIcon(file)
            )}
          </AttachmentMedia>

          {showFileName && (
            <AttachmentContent className="!px-2 !pb-1">
              <AttachmentTitle>{file.filename}</AttachmentTitle>
              <AttachmentDescription>{formatFileSize(file.file_size)}</AttachmentDescription>
            </AttachmentContent>
          )}

          {/* Remove — solid button; a plain tooltip'd Button that opens the
              controlled Confirm (decoupled so the tooltip doesn't flicker). */}
          {(canDelete || canRemove) && onRemove && (
            <AttachmentActions className="!top-1 !right-1 opacity-0 group-hover/attachment:opacity-100 group-focus-within/attachment:opacity-100 hover-none:opacity-100 transition-opacity">
              <Tooltip content="Remove">
                <Button
                  variant="secondary"
                  size="default"
                  icon={<Trash2 />}
                  aria-label="Remove file"
                  data-testid="file-card-remove-btn"
                  onClick={() => setRemoveOpen(true)}
                />
              </Tooltip>
              <Confirm
                open={removeOpen}
                onOpenChange={setRemoveOpen}
                title="Remove this file?"
                description={canDelete ? 'This deletes the file permanently.' : undefined}
                okText="Remove"
                okButtonProps={{ danger: true }}
                cancelText="Cancel"
                onConfirm={() => onRemove()}
                data-testid="file-card-remove-confirm"
              />
            </AttachmentActions>
          )}
        </Attachment>
      </span>
    </Tooltip>
  )
}
