import { Space } from '@/components/ui'
import { CopyButton, DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

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
