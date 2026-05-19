import { Space } from 'antd'
import { CopyButton, DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function TextHeader({ file }: FileViewerSlotProps) {
  return (
    <Space size="small">
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}
