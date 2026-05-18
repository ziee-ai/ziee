import type { FileViewerModule } from '../../types'
import { PdfViewer } from './PdfViewer'

export const viewers: FileViewerModule[] = [
  {
    canHandle: (_, mimeType) => mimeType === 'application/pdf',
    entry: {
      render: props => PdfViewer(props),
      label: 'PDF',
      compilable: false,
      canCopy: false,
    },
  },
]
