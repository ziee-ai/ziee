import { DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function PdfHeader(props: FileViewerSlotProps) {
  if (!('file' in props)) return null
  return <DownloadButton file={props.file} />
}
