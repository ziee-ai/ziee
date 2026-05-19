import { Spin, Typography } from 'antd'
import { FileOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { FileViewRendererProps } from '../../types'

const { Text } = Typography

export function PdfViewer({ file }: FileViewRendererProps) {
  const pageUrls = Stores.Chat.FileStore.getPreviewPageUrls(file)

  if (file.preview_page_count === 0) {
    return (
      <div className="flex flex-col items-center gap-2 py-8">
        <FileOutlined style={{ fontSize: 48 }} />
        <Text type="secondary">Preview not available for this file</Text>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6 p-4 overflow-auto h-full">
      {pageUrls.map((url, i) => (
        <div key={i} className="flex flex-col items-center gap-1">
          <Text type="secondary" className="!text-xs">
            Page {i + 1} of {file.preview_page_count}
          </Text>
          {url
            ? <img src={url} alt={`Page ${i + 1}`} className="w-full object-contain rounded shadow" />
            : <div className="w-full flex items-center justify-center py-16"><Spin /></div>
          }
        </div>
      ))}
    </div>
  )
}
