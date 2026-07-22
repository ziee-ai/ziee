import { lazy } from 'react'
import { FileText } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'

const MarkdownBody = lazy(() => import('./body').then(m => ({ default: m.MarkdownBody })))
const MarkdownHeader = lazy(() => import('./header').then(m => ({ default: m.MarkdownHeader })))

export const viewers: FileViewerModule[] = [
  {
    supportedTypes: [
      { ext: 'md' },
      { ext: 'markdown' },
      { mime: 'text/markdown' },
    ],
    entry: {
      body: MarkdownBody,
      headerActions: MarkdownHeader,
      label: 'Markdown',
      icon: <FileText />,
      // Markdown rendering reuses streamdown's defaults (GFM tables, fenced
      // code, mermaid). Same path as assistant message text rendering —
      // visually consistent across both contexts.
      inline: true,
    },
  },
]
