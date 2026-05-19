import type { FileViewerModule } from '../../types'
import { ImageViewer } from './ImageViewer'

export const viewers: FileViewerModule[] = [
  {
    canHandle: (_, mimeType) => !!(mimeType?.startsWith('image/') && mimeType !== 'image/svg+xml'),
    entry: {
      render: props => ImageViewer(props),
      label: 'Image',
      compilable: false,
      canCopy: false,
    },
  },
]
