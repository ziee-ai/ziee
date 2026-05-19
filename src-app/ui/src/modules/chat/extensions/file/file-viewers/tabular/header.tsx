import { Space } from 'antd'
import { CopyButton, DownloadButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

/** CSV/TSV header — supports raw toggle, copy, download. */
export function DelimitedHeader({ file }: FileViewerSlotProps) {
  return (
    <Space size="small">
      <RawToggle file={file} />
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}

/** XLSX header — binary format, only download. */
export function XlsxHeader({ file }: FileViewerSlotProps) {
  return <DownloadButton file={file} />
}
