import type { FileViewerSlotProps } from '../../types/viewer'

/** PDF has no viewer-specific header chrome. Download is a shared shell-level
 *  affordance rendered by the host (FilePanelHeaderActions). */
export function PdfHeader(_props: FileViewerSlotProps) {
  return null
}
