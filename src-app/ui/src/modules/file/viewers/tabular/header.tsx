import { Space } from 'antd'
import { CopyButton, DownloadButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

/** CSV/TSV header — supports raw toggle, copy, download. */
export function DelimitedHeader(props: FileViewerSlotProps) {
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

/** XLSX header — binary format, only download. */
export function XlsxHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  return <DownloadButton file={props.file} />
}
