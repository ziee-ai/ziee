import { Button, Spin, Typography, theme, App } from 'antd'
import {
  CloseOutlined,
  DeleteOutlined,
  FileOutlined,
  FileTextOutlined,
  FilePdfOutlined,
  PictureOutlined,
  DownloadOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import type { FileUploadProgress } from '@/modules/chat/extensions/file/File.store'
import { getViewer } from '@/modules/chat/extensions/file/fileViewerRegistry'
import { FilePanel } from './FilePanel'

const { Text } = Typography

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

function getFileIcon(file: FileEntity): React.ReactNode {
  if (file.mime_type?.startsWith('image/')) return <PictureOutlined />
  if (file.mime_type === 'application/pdf') return <FilePdfOutlined />
  return <FileTextOutlined />
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
}: FileCardProps) {
  const { token } = theme.useToken()
  const { message } = App.useApp()

  const thumbnailUrls = Stores.Chat.FileStore.thumbnailUrls
  const thumbnailUrl = file ? (thumbnailUrls.get(file.id) ?? null) : null

  const handleCardClick = () => {
    if (!file || uploadProgress) return
    if (onClick) { onClick(); return }

    Stores.Chat.displayInRightPanel({
      id: file.id,
      title: file.filename,
      icon: <FileOutlined />,
      component: () => <FilePanel file={file} />,
    })
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

  // ── Row variant (assistant artifacts) ──────────────────────────────────────
  if (variant === 'row') {
    return (
      <div
        className="w-full flex flex-row items-center gap-3 cursor-pointer rounded-lg p-3 transition-opacity hover:opacity-80"
        style={{
          border: `1px solid ${token.colorBorderSecondary}`,
          backgroundColor: token.colorBgContainer,
        }}
        onClick={handleCardClick}
      >
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

        {/* Right: name + type */}
        <div className="flex flex-col min-w-0 flex-1">
          <Text className="!text-sm font-medium truncate" title={file.filename}>
            {file.filename}
          </Text>
          <Text type="secondary" className="!text-xs truncate">
            {viewerLabel} · {ext}
          </Text>
        </div>

        {/* Download button */}
        <Button
          type="text"
          icon={<DownloadOutlined style={{ fontSize: 20 }} />}
          onClick={e => {
            e.stopPropagation()
            Stores.Chat.FileStore.downloadFile(file)
              .catch(() => message.error('Failed to download file'))
          }}
        />
      </div>
    )
  }

  // ── Square variant (user message attachments & input area) ─────────────────
  return (
    <div className="relative flex flex-col" style={{ width: 96, maxWidth: 96 }}>
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
