import { DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function ImageHeader({ file }: FileViewerSlotProps) {
  return <DownloadButton file={file} />
}
