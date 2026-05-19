import type { FileViewerModule } from '../../types'
import { TextViewer } from './TextViewer'

// Plain text/code extensions — excludes extensions handled by specialized viewers
// (md, markdown, html, htm, svg, csv, tsv, xlsx, xls, ods)
const TEXT_EXTS = new Set([
  'txt', 'json', 'xml', 'yaml', 'yml', 'log', 'ini', 'conf',
  'sh', 'bash', 'py', 'js', 'ts', 'jsx', 'tsx',
  'css', 'scss', 'sql', 'env', 'rs', 'go', 'java',
  'c', 'cpp', 'h', 'rb', 'php', 'swift', 'kt',
  'r', 'lua', 'pl', 'cs', 'dart', 'scala', 'hs',
])

export const viewers: FileViewerModule[] = [
  {
    canHandle: (filename) => TEXT_EXTS.has(filename.split('.').pop()?.toLowerCase() ?? ''),
    entry: {
      render: props => TextViewer(props),
      label: 'Document',
      compilable: false,
      canCopy: true,
    },
  },
]
