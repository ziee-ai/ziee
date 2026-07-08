import { Space } from '@/components/ui'
import { CopyButton, RawToggle } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

/** CSV/TSV header — raw toggle + copy. (Download is a shell-level affordance
 *  rendered by the host — InlineFilePreview / FilePanelHeaderActions.) */
export function DelimitedHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  const { file } = props
  return (
    <Space size="small" wrap={false}>
      <RawToggle file={file} />
      <CopyButton file={file} />
    </Space>
  )
}

/** XLSX header — binary format, no viewer-specific chrome (the host renders the
 *  shared Download affordance). */
export function XlsxHeader(_props: FileViewerSlotProps) {
  return null
}
