import { X, Trash2, FileText, Download, RotateCw } from 'lucide-react'
import { Button, Checkbox, Progress, Spin, Tooltip, Text, message as kitMessage } from '@/components/ui'
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
 * Two variants:
 * - 'row' (default): full-width horizontal card for assistant artifact messages
 * - 'square': compact square card for user message attachments and file input area
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
  // files don't trigger a wasted fetch. Without this, surfaces that
  // hand FileCard a hydrated `file` without separately warming
  // `thumbnailUrls` (e.g. project knowledge files via
  // `ProjectFiles.store.loadFiles`) never get thumbnails.
  if (file && !thumbnailUrl) {
    Stores.File.getThumbnailUrl(file.id, file)
  }

  const handleCardClick = () => {
    if (!file || uploadProgress) return
    if (onClick) { onClick(); return }

    // Default: open the global file-preview drawer. Chat surfaces
    // (composer / message attachments) override `onClick` to open the
    // side-by-side right-panel instead — see FilePreviewList +
    // FileAttachmentRenderer in file/chat-extension/. The drawer is
    // the portable fallback so any non-chat surface (project knowledge
    // drawer, knowledge card on ProjectDetailPage, etc.) gets preview
    // without per-surface plumbing.
    Stores.FilePreviewDrawer.openPreview(file)
  }

  const handleCardKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      handleCardClick()
    }
  }

  // Row-shaped upload-progress branch — added separately from the
  // square branch below (which the chat composer's FilePreviewList
  // depends on; that surface must stay square). Selected by
  // `variant === 'row'` ahead of the legacy square branch.
  if (uploadProgress && variant === 'row') {
    const isError = uploadProgress.status === 'error'
    const percent = Math.round(uploadProgress.progress)
    const ext =
      uploadProgress.filename.split('.').pop()?.toUpperCase() || 'FILE'
    return (
      <div
        className="w-full flex flex-row items-center gap-3 rounded-lg p-3 border border-border"
        data-testid="file-card-uploading"
        data-filename={uploadProgress.filename}
      >
        {/* Left: 40×40 progress ring (or error icon) */}
        <div
          className="flex-shrink-0 flex items-center justify-center"
          style={{ width: 40, height: 40 }}
        >
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
        </div>

        {/* Middle: filename + status / percentage */}
        <div className="flex flex-col min-w-0 flex-1">
          <Text className="!text-sm font-medium truncate" title={uploadProgress.filename}>
            {uploadProgress.filename}
          </Text>
          {isError ? (
            <Text type="secondary" className="!text-xs truncate text-destructive">
              {(uploadProgress as { error?: string }).error ?? 'Upload failed'}
            </Text>
          ) : (
            <Text type="secondary" className="!text-xs truncate">
              Uploading… {percent}%
            </Text>
          )}
        </div>

        {/* Trailing: retry on error if onRetry provided, else cancel */}
        {isError && onRetry ? (
          <Tooltip content="Retry upload">
            <Button
              variant="ghost"
              icon={<RotateCw />}
              onClick={() => onRetry()}
              aria-label={`Retry upload ${uploadProgress.filename}`}
              data-testid="file-card-retry-btn"
            />
          </Tooltip>
        ) : (
          onRemove && (
            <Tooltip content="Dismiss">
              <Button
                variant="ghost"
                icon={<X />}
                onClick={() => onRemove()}
                aria-label={`Dismiss ${uploadProgress.filename}`}
                data-testid="file-card-dismiss-btn"
              />
            </Tooltip>
          )
        )}
      </div>
    )
  }

  if (uploadProgress) {
    return (
      <div
        className="relative flex flex-col w-full h-full"
        data-testid="file-card-uploading"
      >
        <div
          className="group relative rounded min-h-20 min-w-20 max-h-28 max-w-28 w-full h-full flex items-center justify-center border border-border"
        >
          <img
            src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMSIgaGVpZ2h0PSIxIiB2aWV3Qm94PSIwIDAgMSAxIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InRyYW5zcGFyZW50Ii8+PC9zdmc+"
            alt=""
            className="block w-full h-auto opacity-0"
            style={{ aspectRatio: '1' }}
          />
          <div className="absolute inset-0 flex items-center justify-center">
            {uploadProgress.status === 'error' ? (
              onRetry && (
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
              )
            ) : (
              <Spin label="Uploading" />
            )}
          </div>
          {onRemove && (
            <Tooltip
              content={
                uploadProgress.status === 'error' ? 'Dismiss' : 'Cancel upload'
              }
            >
              <Button
                variant="ghost"
                size="default"
                icon={<X />}
                onClick={() => onRemove()}
                className="!absolute top-1 right-1"
                aria-label={
                  uploadProgress.status === 'error'
                    ? `Dismiss ${uploadProgress.filename}`
                    : 'Cancel upload'
                }
                data-testid="file-card-cancel-btn"
              />
            </Tooltip>
          )}
          <Text
            className="absolute top-1 left-1 rounded px-1 !text-[9px] bg-card"
            strong
          >
            {uploadProgress.filename.split('.').pop()?.toUpperCase() || 'FILE'}
          </Text>
          {uploadProgress.status === 'error' && (
            <Text className="absolute top-1 right-1 rounded px-1 !text-[9px] text-white bg-destructive">
              ERROR
            </Text>
          )}
        </div>
        <div
          className="w-full text-center text-xs text-ellipsis overflow-hidden"
          style={{ display: showFileName ? 'block' : 'none' }}
        >
          <Text ellipsis={true} className="whitespace-nowrap !truncate !text-xs">
            {uploadProgress.filename}
          </Text>
        </div>
      </div>
    )
  }

  if (!file) return null

  const isImage = file.mime_type?.startsWith('image/')
  const ext = file.filename.split('.').pop()?.toUpperCase() || 'FILE'
  const viewerLabel = getViewer(file.filename, file.mime_type ?? undefined)?.label ?? ext

  // ── Row variant (assistant artifacts + knowledge management) ───────────────
  if (variant === 'row') {
    return (
      <div
        className="group w-full flex flex-row items-center gap-3 cursor-pointer rounded-lg p-3 border border-border transition-opacity hover:opacity-80 focus-visible:outline focus-visible:outline-2"
        role="button"
        tabIndex={0}
        onClick={handleCardClick}
        onKeyDown={handleCardKeyDown}
        data-testid="file-card"
        data-file-id={file.id}
        data-filename={file.filename}
      >
        {/* Optional multi-select checkbox — always visible when
            `selectable` so users see it without having to hover.
            Stop propagation so the outer card-click doesn't fire
            alongside the checkbox toggle. */}
        {selectable && (
          <div
            className="flex-shrink-0"
            onClick={e => e.stopPropagation()}
          >
            <Checkbox
              checked={selected}
              onChange={checked => onSelectChange?.(checked)}
              aria-label={`Select ${file.filename}`}
              data-testid="file-card-select-checkbox"
            />
          </div>
        )}

        {/* Left: icon box */}
        <div
          className="flex-shrink-0 flex items-center justify-center rounded-lg overflow-hidden bg-muted text-muted-foreground"
          style={{
            width: 40,
            height: 40,
            fontSize: 20,
          }}
        >
          {isImage && thumbnailUrl ? (
            <img src={thumbnailUrl} alt={file.filename} className="w-full h-full object-cover" />
          ) : (
            getFileIcon(file)
          )}
        </div>

        {/* Middle: name + subtitle (caller can override) */}
        <div className="flex flex-col min-w-0 flex-1">
          <Text className="!text-sm font-medium truncate" title={file.filename}>
            {file.filename}
          </Text>
          <Text type="secondary" className="!text-xs truncate">
            {subtitle ?? <>{viewerLabel} · {ext}</>}
          </Text>
        </div>

        {/* Trailing: caller-provided actions OR default Download button.
            actions slot wins — knowledge panels pass a Confirm-wrapped
            delete here; chat callers pass nothing and get Download. */}
        {actions !== undefined ? (
          <div onClick={e => e.stopPropagation()} className="flex-shrink-0">
            {actions}
          </div>
        ) : (
          canDownload && (
            <Tooltip content="Download">
              <Button
                variant="ghost"
                icon={<Download style={{ fontSize: 20 }} />}
                aria-label={`Download ${file.filename}`}
                data-testid="file-card-download-btn"
                onClick={e => {
                  e.stopPropagation()
                  Stores.File.downloadFile(file)
                    .catch(() => kitMessage.error('Failed to download file'))
                }}
              />
            </Tooltip>
          )
        )}
      </div>
    )
  }

  // ── Square variant (user message attachments & input area) ─────────────────
  // When `stretch` is false (the default — chat composer's
  // FilePreviewList), the wrapper is a fixed 96 px square. When the
  // caller opts into `stretch`, we drop the fixed width so the parent
  // grid controls the size; the `aspect-ratio: 1` enforcer below keeps
  // the card square at whatever width it receives.
  return (
    <div
      className={`relative flex flex-col ${stretch ? 'w-full' : ''}`}
      style={stretch ? undefined : { width: 96, maxWidth: 96 }}
      data-testid="file-card"
      data-file-id={file.id}
      data-filename={file.filename}
    >
      <div
        className={`group relative cursor-pointer rounded-2xl w-full h-full flex items-center justify-center focus-visible:outline focus-visible:outline-2 ${
          stretch ? '' : 'min-h-20 min-w-20 max-h-28 max-w-28'
        } border border-border`}
        style={{
          ...(isImage && thumbnailUrl
            ? {
                backgroundImage: `url(${thumbnailUrl})`,
                backgroundSize: 'cover',
                backgroundPosition: 'center',
                backgroundRepeat: 'no-repeat',
              }
            : {}),
        }}
        role="button"
        tabIndex={0}
        onClick={handleCardClick}
        onKeyDown={handleCardKeyDown}
      >
        {/* Square aspect ratio enforcer */}
        <img
          src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMSIgaGVpZ2h0PSIxIiB2aWV3Qm94PSIwIDAgMSAxIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InRyYW5zcGFyZW50Ii8+PC9zdmc+"
          alt=""
          className="block w-full h-auto opacity-0"
          style={{ aspectRatio: '1' }}
        />

        {/* Centered file type icon (for non-images) */}
        {!isImage && (
          <div
            className="absolute inset-0 flex items-center justify-center text-muted-foreground"
            style={{ fontSize: 28 }}
          >
            {getFileIcon(file)}
          </div>
        )}

        {/* Delete/Remove button — Confirm-wrapped so the
            destructive action requires explicit confirmation.
            The row variant routes its delete through the `actions`
            slot (caller already wraps in Confirm); the square
            variant has its delete inline, so the confirm lives
            here.

            stopPropagation is on the OUTER wrapper, not the Button.
            If we put stopPropagation on the Button itself, it
            cancels the click event before it bubbles up to
            Tooltip's wrapper span — which is where Confirm
            attaches its trigger handler — so the popover never
            opens. Hoisting it to a wrapper that lives outside
            Confirm's trigger subtree means Confirm's
            handler runs first (popover opens), then our handler
            stops the click from reaching the card's onClick. */}
        {(canDelete || canRemove) && onRemove && (
          <div
            className="!absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity"
            style={{ display: canRemove ? 'block' : 'none' }}
            onClick={e => e.stopPropagation()}
          >
            <Confirm
              title="Remove this file?"
              description={canDelete ? 'This deletes the file permanently.' : undefined}
              okText="Remove"
              okButtonProps={{ danger: true }}
              cancelText="Cancel"
              onConfirm={() => onRemove()}
              data-testid="file-card-remove-confirm"
            >
              <Tooltip content="Remove">
                <Button
                  variant="outline"
                  size="default"
                  icon={<Trash2 />}
                  aria-label="Remove file"
                  className="bg-transparent"
                  data-testid="file-card-remove-btn"
                />
              </Tooltip>
            </Confirm>
          </div>
        )}

        {/* File size badge */}
        <Text
          className="absolute bottom-1 right-1 rounded px-1 !text-[9px] bg-card"
        >
          {formatFileSize(file.file_size)}
        </Text>
      </div>

      <div
        className="w-full text-center text-xs text-ellipsis overflow-hidden"
        style={{ display: showFileName ? 'block' : 'none' }}
      >
        <Text ellipsis={true} className="whitespace-nowrap !truncate !text-xs">
          {file.filename}
        </Text>
      </div>
    </div>
  )
}
