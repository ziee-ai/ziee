import { Button, Checkbox, Progress, Spin, Typography, theme, App } from 'antd'
import {
  CloseOutlined,
  DeleteOutlined,
  FileTextOutlined,
  DownloadOutlined,
  ReloadOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type File as FileEntity } from '@/api-client/types'
import type { FileUploadProgress } from '@/modules/file/stores/File.store'
import { getViewer } from '@/modules/file/registry/fileViewerRegistry'

const { Text } = Typography

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
    ?? <FileTextOutlined />
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
  /** Row only — appended to the trailing edge in place of the default
   *  Download button. Use this slot to pass a Popconfirm-wrapped
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
  actions,
  subtitle,
  selectable = false,
  selected = false,
  onSelectChange,
  onRetry,
}: FileCardProps) {
  const { token } = theme.useToken()
  const { message } = App.useApp()
  const canDownload = usePermission(Permissions.FilesDownload)

  const thumbnailUrls = Stores.File.thumbnailUrls
  const thumbnailUrl = file ? (thumbnailUrls.get(file.id) ?? null) : null

  const handleCardClick = () => {
    if (!file || uploadProgress) return
    if (onClick) { onClick(); return }

    Stores.Chat.displayInRightPanel({
      id: file.id,
      title: file.filename,
      type: 'file',
      data: { fileId: file.id },
    })
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
        className="w-full flex flex-row items-center gap-3 rounded-lg p-3"
        style={{
          border: `1px solid ${isError ? token.colorErrorBorder : token.colorBorderSecondary}`,
          backgroundColor: token.colorBgContainer,
        }}
        data-testid="file-card-uploading"
        data-filename={uploadProgress.filename}
      >
        {/* Left: 40×40 progress ring (or error icon) */}
        <div
          className="flex-shrink-0 flex items-center justify-center"
          style={{ width: 40, height: 40 }}
        >
          {isError ? (
            <Text
              className="!text-[9px] rounded px-1"
              style={{ backgroundColor: token.colorError, color: token.colorWhite }}
            >
              ERROR
            </Text>
          ) : (
            <Progress
              type="circle"
              percent={percent}
              size={32}
              strokeWidth={10}
              format={() => ext}
            />
          )}
        </div>

        {/* Middle: filename + status / percentage */}
        <div className="flex flex-col min-w-0 flex-1">
          <Text className="!text-sm font-medium truncate" title={uploadProgress.filename}>
            {uploadProgress.filename}
          </Text>
          {isError ? (
            <Text type="danger" className="!text-xs truncate">
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
          <Button
            type="text"
            icon={<ReloadOutlined />}
            onClick={() => onRetry()}
            aria-label={`Retry upload ${uploadProgress.filename}`}
          />
        ) : (
          onRemove && (
            <Button
              type="text"
              icon={<CloseOutlined />}
              onClick={() => onRemove()}
              aria-label={`Dismiss ${uploadProgress.filename}`}
            />
          )
        )}
      </div>
    )
  }

  if (uploadProgress) {
    return (
      <div className="relative flex flex-col w-full h-full">
        <div
          className="group relative rounded min-h-20 min-w-20 max-h-28 max-w-28 w-full h-full flex items-center justify-center"
          style={{
            border: `1px solid ${token.colorBorderSecondary}`,
            backgroundColor: token.colorBgContainer,
          }}
        >
          <img
            src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMSIgaGVpZ2h0PSIxIiB2aWV3Qm94PSIwIDAgMSAxIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InRyYW5zcGFyZW50Ii8+PC9zdmc+"
            alt=""
            className="block w-full h-auto opacity-0"
            style={{ aspectRatio: '1' }}
          />
          <div className="absolute inset-0 flex items-center justify-center">
            <Spin />
          </div>
          {onRemove && (
            <Button
              danger
              size="small"
              icon={<CloseOutlined />}
              onClick={() => onRemove()}
              className="!absolute top-1 right-1"
            />
          )}
          <Text
            className="absolute top-1 left-1 rounded px-1 !text-[9px]"
            style={{ backgroundColor: token.colorBgContainer }}
            strong
          >
            {uploadProgress.filename.split('.').pop()?.toUpperCase() || 'FILE'}
          </Text>
          {uploadProgress.status === 'error' && (
            <Text
              className="absolute top-1 right-1 rounded px-1 !text-[9px]"
              style={{ backgroundColor: token.colorError, color: token.colorWhite }}
            >
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
        className="group w-full flex flex-row items-center gap-3 cursor-pointer rounded-lg p-3 transition-opacity hover:opacity-80"
        style={{
          border: `1px solid ${token.colorBorderSecondary}`,
          backgroundColor: token.colorBgContainer,
        }}
        onClick={handleCardClick}
        data-testid="file-card"
        data-file-id={file.id}
        data-filename={file.filename}
      >
        {/* Optional multi-select checkbox — hover-revealed when
            unselected, always-visible when selected. Mirrors the
            ConversationCard pattern. Stop propagation so the outer
            card-click doesn't fire alongside the checkbox toggle. */}
        {selectable && (
          <div
            className={`flex-shrink-0 transition-opacity ${selected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}
            onClick={e => e.stopPropagation()}
          >
            <Checkbox
              checked={selected}
              onChange={e => onSelectChange?.(e.target.checked)}
              aria-label={`Select ${file.filename}`}
            />
          </div>
        )}

        {/* Left: icon box */}
        <div
          className="flex-shrink-0 flex items-center justify-center rounded-lg overflow-hidden"
          style={{
            width: 40,
            height: 40,
            backgroundColor: token.colorFillTertiary,
            fontSize: 20,
            color: token.colorTextSecondary,
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
            actions slot wins — knowledge panels pass a Popconfirm-wrapped
            delete here; chat callers pass nothing and get Download. */}
        {actions !== undefined ? (
          <div onClick={e => e.stopPropagation()} className="flex-shrink-0">
            {actions}
          </div>
        ) : (
          canDownload && (
            <Button
              type="text"
              icon={<DownloadOutlined style={{ fontSize: 20 }} />}
              onClick={e => {
                e.stopPropagation()
                Stores.File.downloadFile(file)
                  .catch(() => message.error('Failed to download file'))
              }}
            />
          )
        )}
      </div>
    )
  }

  // ── Square variant (user message attachments & input area) ─────────────────
  return (
    <div
      className="relative flex flex-col"
      style={{ width: 96, maxWidth: 96 }}
      data-testid="file-card"
      data-file-id={file.id}
      data-filename={file.filename}
    >
      <div
        className="group relative cursor-pointer rounded-2xl min-h-20 min-w-20 max-h-28 max-w-28 w-full h-full flex items-center justify-center"
        style={{
          border: `1px solid ${token.colorBorderSecondary}`,
          backgroundColor: token.colorBgContainer,
          ...(isImage && thumbnailUrl
            ? {
                backgroundImage: `url(${thumbnailUrl})`,
                backgroundSize: 'cover',
                backgroundPosition: 'center',
                backgroundRepeat: 'no-repeat',
              }
            : {}),
        }}
        onClick={handleCardClick}
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
            className="absolute inset-0 flex items-center justify-center"
            style={{ fontSize: 28, color: token.colorTextTertiary }}
          >
            {getFileIcon(file)}
          </div>
        )}

        {/* Delete/Remove button - only visible on hover */}
        {(canDelete || canRemove) && onRemove && (
          <Button
            danger
            size="small"
            icon={<DeleteOutlined />}
            onClick={e => {
              e.stopPropagation()
              onRemove()
            }}
            style={{ display: canRemove ? 'block' : 'none' }}
            className="!absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity bg-transparent"
          />
        )}

        {/* File size badge */}
        <Text
          className="absolute bottom-1 right-1 rounded px-1 !text-[9px]"
          style={{ backgroundColor: token.colorBgContainer }}
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
