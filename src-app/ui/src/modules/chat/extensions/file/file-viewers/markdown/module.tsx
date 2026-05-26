import type { FileViewerModule } from '../../types'
import { FileMarkdownOutlined } from '@ant-design/icons'
import { MarkdownBody } from './body'
import { MarkdownHeader } from './header'

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
      icon: <FileMarkdownOutlined />,
      // Markdown rendering reuses streamdown's defaults (GFM tables, fenced
      // code, mermaid). Same path as assistant message text rendering —
      // visually consistent across both contexts.
      inline: true,
    },
  },
]
