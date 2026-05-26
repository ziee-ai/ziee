import { DownloadButton } from '../shared/chrome'
import type { FileViewerSlotProps } from '../../types'

export function ImageHeader(props: FileViewerSlotProps) {
  // Inline-context headers are owned by the chat-side InlineFilePreview
  // (which renders icon + filename + open-in-new-tab link). The
  // FileStore-coupled chrome below only works with a real FileEntity,
  // so for inline context we render nothing extra.
  if (!('file' in props)) return null
  return <DownloadButton file={props.file} />
}
