import type { FileViewerModule } from '../../types'
import { WebViewer } from './WebViewer'

function ext(filename: string) {
  return filename.split('.').pop()?.toLowerCase() ?? ''
}

export const viewers: FileViewerModule[] = [
  {
    canHandle: (filename, mimeType) =>
      ext(filename) === 'html' || ext(filename) === 'htm' || mimeType === 'text/html',
    entry: {
      render: props => WebViewer(props),
      label: 'HTML',
      compilable: true,
      canCopy: true,
    },
  },
  {
    canHandle: (filename, mimeType) =>
      ext(filename) === 'svg' || mimeType === 'image/svg+xml',
    entry: {
      render: props => WebViewer(props),
      label: 'SVG',
      compilable: true,
      canCopy: true,
    },
  },
]
