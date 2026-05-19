import { DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function PdfHeader({ file }: FileViewerSlotProps) {
  return <DownloadButton file={file} />
}
