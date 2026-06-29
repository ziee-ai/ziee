import { Space } from '@/components/ui'
import { CopyButton, DownloadButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

export function WebHeader(props: FileViewerSlotProps) {
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
