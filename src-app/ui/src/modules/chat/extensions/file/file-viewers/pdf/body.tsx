import { Spin, Typography } from 'antd'
import { FileOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types'

const { Text } = Typography

export function PdfBody({ file }: FileViewerSlotProps) {
  // Subscribe to previewPageUrls Map directly so we re-render as each
  // page slot loads. Calling the `getPreviewPageUrls()` action instead
  // would only subscribe to the function reference (whose identity never
  // changes), so the body would freeze at the initial placeholder array.
  const previewPageUrls = Stores.Chat.FileStore.previewPageUrls
  const cachedUrls = previewPageUrls.get(file.id)
  const pageUrls = cachedUrls ?? Stores.Chat.FileStore.getPreviewPageUrls(file)

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
