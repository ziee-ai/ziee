import { Space } from 'antd'
import { CopyButton, DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function TextHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  const { file } = props
  return (
    <Space size="small">
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}
