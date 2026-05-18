import type { FileViewerModule } from '../../types'
import { MarkdownViewer } from './MarkdownViewer'

const MD_EXTS = new Set(['md', 'markdown'])

export const viewers: FileViewerModule[] = [
  {
    canHandle: (filename) => MD_EXTS.has(filename.split('.').pop()?.toLowerCase() ?? ''),
    entry: {
      render: props => MarkdownViewer(props),
      label: 'Markdown',
      compilable: true,
      canCopy: true,
    },
  },
]
