import { Space } from 'antd'
import { CopyButton, DownloadButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function MarkdownHeader(props: FileViewerSlotProps) {
  // Chrome buttons read from the FileStore via `file.id`; they need
  // a real FileEntity. Inline context renders no extra header chrome.
  if (!('file' in props)) return null
  const { file } = props
  return (
    <Space size="small">
      <RawToggle file={file} />
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}
