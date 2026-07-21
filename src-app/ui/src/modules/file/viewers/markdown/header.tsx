import { Space } from '@ziee/kit'
import {
  CopyButton,
  CopySelectionButton,
  FindButton,
  RawToggle,
  WrapToggle,
} from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'
import { File } from '@/modules/file/stores/file'

export function MarkdownHeader(props: FileViewerSlotProps) {
  // Chrome buttons read from the FileStore via `file.id`; they need
  // a real FileEntity. Inline context renders no extra header chrome.
  if (!('file' in props)) return null
  const { file } = props
  // Word-wrap only applies to the raw (RawCodeView) mode; the rendered markdown
  // already wraps. Show the toggle only when raw is active.
  const isRaw = (File.fileViewModes.get(file.id) ?? 'compiled') === 'raw'
  return (
    <Space size="small">
      <FindButton file={file} />
      {isRaw ? <WrapToggle file={file} /> : null}
      <RawToggle file={file} />
      <CopySelectionButton />
      <CopyButton file={file} />
    </Space>
  )
}
