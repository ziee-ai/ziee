import { Space } from '@ziee/kit'
import {
  CopyButton,
  CopySelectionButton,
  FindButton,
  WrapToggle,
} from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types/viewer'

export function TextHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  const { file } = props
  return (
    <Space size="small">
      <FindButton file={file} />
      <WrapToggle file={file} />
      <CopySelectionButton />
      <CopyButton file={file} />
    </Space>
  )
}
