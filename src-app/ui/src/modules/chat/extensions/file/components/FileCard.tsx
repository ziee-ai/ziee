import { useState, useEffect } from 'react'
import { Button, Spin, Typography, theme } from 'antd'
import { CloseOutlined, DeleteOutlined } from '@ant-design/icons'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'
import type { FileUploadProgress } from '../File.store'

const { Text } = Typography

/**
 * Format file size for display
 */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export interface FileCardProps {
  // Either a completed file or an uploading file
  file?: FileEntity
  uploadProgress?: FileUploadProgress

  // Actions
  onRemove?: () => void
  onDownload?: () => void
  onClick?: () => void

  // Display options
  showFileName?: boolean
  canRemove?: boolean
  canDelete?: boolean
}

/**
 * FileCard Component
 * Square thumbnail card matching reference implementation
 */
export function FileCard({
  file,
  uploadProgress,
  onRemove,
  onClick,
  showFileName = true,
  canRemove = true,
  canDelete = false,
}: FileCardProps) {
  const { token } = theme.useToken()
  const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(null)

  // Load thumbnail for files that have previews
  useEffect(() => {
    if (!file || !file.has_thumbnail || file.preview_page_count === 0) {
      return
    }

    let isMounted = true
    let objectUrl: string | null = null

    const loadThumbnail = async () => {
      try {
        const response = await ApiClient.File.getPreview({
          file_id: file.id,
          page: 1,
        })
        objectUrl = window.URL.createObjectURL(response)
        if (isMounted) {
          setThumbnailUrl(objectUrl)
        }
      } catch (error) {
        console.debug('Failed to load thumbnail:', error)
      }
    }

    loadThumbnail()

    // Cleanup function to revoke object URL and prevent memory leaks
    return () => {
      isMounted = false
      if (objectUrl) {
        window.URL.revokeObjectURL(objectUrl)
      }
    }
  }, [file?.id])

  // Uploading state
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
          {/* Square aspect ratio enforcer - invisible 1x1 base64 image */}
          <img
            src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMSIgaGVpZ2h0PSIxIiB2aWV3Qm94PSIwIDAgMSAxIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InRyYW5zcGFyZW50Ii8+PC9zdmc+"
            alt=""
            className="block w-full h-auto opacity-0"
            style={{ aspectRatio: '1' }}
          />

          {/* Spinner - centered */}
          <div className="absolute inset-0 flex items-center justify-center">
            <Spin />
          </div>

          {/* Remove button for uploading files */}
          {onRemove && (
            <Button
              danger
              size="small"
              icon={<CloseOutlined />}
              onClick={() => onRemove()}
              className="!absolute top-1 right-1"
            />
          )}

          {/* File extension */}
          <Text
            className="absolute top-1 left-1 rounded px-1 !text-[9px]"
            style={{
              backgroundColor: token.colorBgContainer,
            }}
            strong
          >
            {uploadProgress.filename.split('.').pop()?.toUpperCase() || 'FILE'}
          </Text>

          {/* Upload status */}
          {uploadProgress.status === 'error' && (
            <Text
              className="absolute top-1 right-1 rounded px-1 !text-[9px]"
              style={{
                backgroundColor: token.colorError,
                color: token.colorWhite,
              }}
            >
              ERROR
            </Text>
          )}
        </div>
        <div
          className="w-full text-center text-xs text-ellipsis overflow-hidden"
          style={{
            display: showFileName ? 'block' : 'none',
          }}
        >
          <Text ellipsis={true} className="whitespace-nowrap !truncate !text-xs">
            {uploadProgress.filename}
          </Text>
        </div>
      </div>
    )
  }

  if (!file) {
    return null
  }

  return (
    <div className="relative flex flex-col w-full h-full">
      <div
        className="group relative cursor-pointer rounded min-h-20 min-w-20 max-h-28 max-w-28 w-full h-full"
        style={{
          border: `1px solid ${token.colorBorderSecondary}`,
          backgroundColor: token.colorBgContainer,
          backgroundImage: thumbnailUrl ? `url(${thumbnailUrl})` : undefined,
          backgroundSize: 'cover',
          backgroundPosition: 'center',
          backgroundRepeat: 'no-repeat',
        }}
        onClick={onClick}
      >
        {/* Square aspect ratio enforcer - invisible 1x1 base64 image */}
        <img
          src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMSIgaGVpZ2h0PSIxIiB2aWV3Qm94PSIwIDAgMSAxIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InRyYW5zcGFyZW50Ii8+PC9zdmc+"
          alt=""
          className="block w-full h-auto opacity-0"
          style={{ aspectRatio: '1' }}
        />

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
            style={{
              display: canRemove ? 'block' : 'none',
            }}
            className="!absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity bg-transparent"
          />
        )}

        {/* File extension badge */}
        <Text
          className="absolute top-1 left-1 rounded px-1 !text-[9px]"
          style={{
            backgroundColor: token.colorBgContainer,
          }}
          strong
        >
          {file.filename.split('.').pop()?.toUpperCase() || 'FILE'}
        </Text>

        {/* File size badge */}
        <Text
          className="absolute bottom-1 right-1 rounded px-1 !text-[9px]"
          style={{
            backgroundColor: token.colorBgContainer,
          }}
        >
          {formatFileSize(file.file_size)}
        </Text>
      </div>
      <div
        className="w-full text-center text-xs text-ellipsis overflow-hidden"
        style={{
          display: showFileName ? 'block' : 'none',
        }}
      >
        <Text ellipsis={true} className="whitespace-nowrap !truncate !text-xs">
          {file.filename}
        </Text>
      </div>
    </div>
  )
}
