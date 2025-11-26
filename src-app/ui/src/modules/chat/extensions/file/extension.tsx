import { Card, Typography, Button } from 'antd'
import {
  FileOutlined,
  DownloadOutlined,
  FileImageOutlined,
  FilePdfOutlined,
  FileTextOutlined,
  FileZipOutlined,
} from '@ant-design/icons'
import {
  createExtension,
  
  type ChatExtension,
  type ContentRendererProps,
} from '../../core/extensions'

const { Text } = Typography

/**
 * File attachment data structure
 */
interface FileAttachment {
  id: string
  name: string
  size: number
  mime_type: string
  url?: string
}

/**
 * Get appropriate icon for file type
 */
function getFileIcon(mimeType: string) {
  if (mimeType.startsWith('image/')) {
    return <FileImageOutlined className="text-blue-500" />
  }
  if (mimeType === 'application/pdf') {
    return <FilePdfOutlined className="text-red-500" />
  }
  if (mimeType.startsWith('text/')) {
    return <FileTextOutlined className="text-green-500" />
  }
  if (
    mimeType.includes('zip') ||
    mimeType.includes('compressed') ||
    mimeType.includes('archive')
  ) {
    return <FileZipOutlined className="text-orange-500" />
  }
  return <FileOutlined />
}

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
 * File Attachment UI Component
 */
function FileAttachmentUI({ file }: { file: FileAttachment }) {
  const handleDownload = () => {
    if (file.url) {
      window.open(file.url, '_blank')
    }
  }

  // If it's an image, show preview
  if (file.mime_type.startsWith('image/') && file.url) {
    return (
      <Card
        size="small"
        className="mb-2"
        cover={
          <img
            alt={file.name}
            src={file.url}
            className="max-h-64 object-contain"
          />
        }
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {getFileIcon(file.mime_type)}
            <div>
              <Text strong className="block">
                {file.name}
              </Text>
              <Text type="secondary" className="text-xs">
                {formatFileSize(file.size)}
              </Text>
            </div>
          </div>
          {file.url && (
            <Button
              size="small"
              icon={<DownloadOutlined />}
              onClick={handleDownload}
            >
              Download
            </Button>
          )}
        </div>
      </Card>
    )
  }

  // For other file types, show compact card
  return (
    <Card
      size="small"
      className="mb-2"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.02)' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {getFileIcon(file.mime_type)}
          <div>
            <Text strong className="block">
              {file.name}
            </Text>
            <Text type="secondary" className="text-xs">
              {formatFileSize(file.size)}
            </Text>
          </div>
        </div>
        {file.url && (
          <Button
            size="small"
            icon={<DownloadOutlined />}
            onClick={handleDownload}
          >
            Download
          </Button>
        )}
      </div>
    </Card>
  )
}

/**
 * File attachment content renderer component
 */
function FileAttachmentRenderer({ content }: ContentRendererProps) {
  const fileData = content.content as FileAttachment

  if (!fileData?.name) {
    return null
  }

  return <FileAttachmentUI file={fileData} />
}

/**
 * File Extension
 * Handles file attachment rendering in messages
 */
const fileExtension: ChatExtension = createExtension({
  name: 'file',
  description: 'Handles file attachment rendering',
  priority: 80,

  // No per-conversation state needed

  // Register content type components
  contentTypes: {
    file_attachment: FileAttachmentRenderer,
  },
})

export default fileExtension
