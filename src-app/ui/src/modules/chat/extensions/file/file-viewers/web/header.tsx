import { Space } from 'antd'
import { CopyButton, DownloadButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function WebHeader({ file }: FileViewerSlotProps) {
  return (
    <Space size="small">
      <RawToggle file={file} />
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}
