import { Space } from '@/components/ui'
import { Stores } from '@/core/stores'
import {
  CopyButton,
  CopySelectionButton,
  DownloadButton,
  FindButton,
  RawToggle,
  WrapToggle,
} from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

export function WebHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  const { file } = props
  const isRaw = (Stores.File.fileViewModes.get(file.id) ?? 'compiled') === 'raw'
  return (
    <Space size="small">
      {/* Find + copy-selection operate on the raw text; the rendered iframe is a
          separate document our highlight/selection can't reach, so they're most
          useful in raw mode but the buttons stay available for the raw toggle. */}
      {isRaw ? <FindButton file={file} /> : null}
      {isRaw ? <WrapToggle file={file} /> : null}
      {isRaw ? <CopySelectionButton /> : null}
      <RawToggle file={file} />
      <CopyButton file={file} />
      <DownloadButton file={file} />
    </Space>
  )
}
