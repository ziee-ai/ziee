import { useState, useEffect } from 'react'
import { Button, Spin, Typography, theme, App, Drawer, Card } from 'antd'
import { CloseOutlined, DeleteOutlined, DownloadOutlined } from '@ant-design/icons'
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

/**
 * Check if file is a text file based on extension
 */
function isTextFile(filename: string): boolean {
  const textExtensions = [
    'txt',
    'md',
    'json',
    'xml',
    'yaml',
    'yml',
    'csv',
    'log',
    'ini',
    'conf',
    'sh',
    'bash',
    'py',
    'js',
    'ts',
    'jsx',
    'tsx',
    'html',
    'css',
    'scss',
    'sql',
    'env',
  ]
  const ext = filename.split('.').pop()?.toLowerCase() || ''
  return textExtensions.includes(ext)
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

interface FileModalContentProps {
  file: FileEntity
}

/**
 * File preview modal content showing thumbnails and download button
 */
function FileModalContent({ file }: FileModalContentProps) {
  const [thumbnails, setThumbnails] = useState<string[]>([])
  const [thumbnailOrder, setThumbnailOrder] = useState<number[]>([])
  const [loading, setLoading] = useState(true)
  const [isAnimating, setIsAnimating] = useState(false)
  const { message } = App.useApp()

  // Load all page thumbnails
  useEffect(() => {
    console.log('[FileModalContent] Loading thumbnails for:', {
      filename: file.filename,
      preview_page_count: file.preview_page_count,
    })

    if (file.preview_page_count === 0) {
      console.log('[FileModalContent] No preview pages available')
      setLoading(false)
      return
    }

    let isMounted = true
    const urls: string[] = []

    const loadThumbnails = async () => {
      setLoading(true)
      try {
        for (let page = 1; page <= file.preview_page_count; page++) {
          console.log('[FileModalContent] Loading page:', page)
          const response = await ApiClient.File.getPreview({
            file_id: file.id,
            page,
          })
          const objectUrl = window.URL.createObjectURL(response)
          urls.push(objectUrl)
        }
        if (isMounted) {
          setThumbnails(urls)
          setThumbnailOrder(Array.from({ length: urls.length }, (_, i) => i))
        }
      } catch (error) {
        console.debug('Failed to load thumbnails:', error)
        message.error('Failed to load file preview')
      } finally {
        if (isMounted) {
          setLoading(false)
        }
      }
    }

    loadThumbnails()

    return () => {
      isMounted = false
      urls.forEach(url => window.URL.revokeObjectURL(url))
    }
  }, [file.id, file.preview_page_count])

  const handleThumbnailClick = () => {
    if (thumbnailOrder.length <= 1 || isAnimating) return

    setIsAnimating(true)

    setTimeout(() => {
      const newOrder = [...thumbnailOrder]
      const frontIndex = newOrder.shift()
      if (frontIndex !== undefined) {
        newOrder.push(frontIndex)
      }
      setThumbnailOrder(newOrder)
    }, 50)

    setTimeout(() => {
      setIsAnimating(false)
    }, 350)
  }

  const handleDownload = async () => {
    try {
      const response = await ApiClient.File.download({ file_id: file.id })
      const blob = response instanceof Blob ? response : new Blob([response])
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = file.filename
      document.body.appendChild(a)
      a.click()
      window.URL.revokeObjectURL(url)
      document.body.removeChild(a)
    } catch (error) {
      console.error('Failed to download file:', error)
      message.error('Failed to download file')
    }
  }

  return (
    <div className="flex flex-col items-center gap-4 py-4">
      <div className="text-center">
        {loading ? (
          <div className="text-6xl mb-4">⏳</div>
        ) : thumbnails.length > 0 ? (
          <div className="mb-4 relative">
            <div
              className="relative group"
              style={{ width: 'fit-content', margin: '0 auto' }}
              onClick={handleThumbnailClick}
              title={
                thumbnailOrder.length > 1
                  ? 'Click to cycle through thumbnails'
                  : ''
              }
            >
              {thumbnailOrder.map((originalIndex, displayIndex) => (
                <img
                  key={`${originalIndex}-${displayIndex}`}
                  src={thumbnails[originalIndex]}
                  alt={`${file.filename} - Page ${originalIndex + 1}`}
                  className="max-w-full max-h-96 object-contain rounded shadow transition-all duration-300 ease-in-out hover:scale-105"
                  style={{
                    position: displayIndex === 0 ? 'relative' : 'absolute',
                    top: displayIndex === 0 ? 0 : `${displayIndex * 8}px`,
                    left: displayIndex === 0 ? 0 : `${displayIndex * 8}px`,
                    zIndex: thumbnailOrder.length - displayIndex,
                    transform: `${displayIndex > 0 ? 'rotate(2deg)' : 'none'} ${
                      isAnimating && displayIndex === 0
                        ? 'scale(0.95) translateY(-5px)'
                        : ''
                    }`,
                    opacity: isAnimating && displayIndex === 0 ? 0.8 : 1,
                  }}
                />
              ))}
            </div>
          </div>
        ) : (
          <div>
            <div className="text-6xl mb-4">📄</div>
            <Text type="secondary">
              Preview not available for this file type
            </Text>
          </div>
        )}
        <div className="pt-4">
          <Text type="secondary">
            File size: {formatFileSize(file.file_size)}
          </Text>
        </div>
      </div>
      <Button type="primary" icon={<DownloadOutlined />} onClick={handleDownload}>
        Download File
      </Button>
    </div>
  )
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
  const { modal, message } = App.useApp()
  const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(null)
  const [isDrawerOpen, setIsDrawerOpen] = useState(false)
  const [fileContent, setFileContent] = useState<string>('')

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

  // Handle card click to show preview or download
  const handleCardClick = async () => {
    if (!file || uploadProgress) return

    console.log('[FileCard] Click - file:', {
      filename: file.filename,
      has_thumbnail: file.has_thumbnail,
      preview_page_count: file.preview_page_count,
      mime_type: file.mime_type,
    })

    // If custom onClick is provided, use that instead
    if (onClick) {
      onClick()
      return
    }

    // For text files, show content in drawer
    if (isTextFile(file.filename)) {
      try {
        const response = await ApiClient.File.getTextContent({ file_id: file.id })
        // Response is a Blob, convert to text
        const text = await response.text()
        setFileContent(text)
        setIsDrawerOpen(true)
      } catch (error) {
        console.error('Failed to fetch file content:', error)
        message.error('Failed to load file content')
      }
    } else {
      // For other files, show modal with thumbnails
      modal.info({
        icon: null,
        title: file.filename,
        width: 600,
        content: <FileModalContent file={file} />,
        footer: null,
        closable: true,
        maskClosable: true,
        styles: {
          body: {
            backgroundColor: token.colorBgLayout,
            border: `1px solid ${token.colorBorderSecondary}`,
          },
        },
      })
    }
  }

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
    <>
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
          onClick={handleCardClick}
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

      {/* Drawer for text file content */}
      <Drawer
        title={file.filename}
        open={isDrawerOpen}
        onClose={() => setIsDrawerOpen(false)}
        size={600}
        classNames={{
          body: '!px-3 !pt-0',
        }}
      >
        <Card className="font-mono text-sm whitespace-pre-wrap p-4 rounded max-h-full overflow-auto">
          {fileContent}
        </Card>
      </Drawer>
    </>
  )
}
