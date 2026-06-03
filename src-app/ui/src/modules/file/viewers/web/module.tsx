import type { FileViewerModule } from '../../types/viewer'
import { Html5Outlined, FileImageOutlined } from '@ant-design/icons'
import { WebBody } from './body'
import { WebHeader } from './header'

export const viewers: FileViewerModule[] = [
  {
    supportedTypes: [
      { ext: 'html' },
      { ext: 'htm' },
      { mime: 'text/html' },
    ],
    entry: {
      body: WebBody,
      headerActions: WebHeader,
      label: 'HTML',
      icon: <Html5Outlined />,
    },
  },
  {
    // Default priority 0 — beats image/* (priority 10) from the image viewer.
    supportedTypes: [
      { ext: 'svg' },
      { mime: 'image/svg+xml' },
    ],
    entry: {
      body: WebBody,
      headerActions: WebHeader,
      label: 'SVG',
      icon: <FileImageOutlined />,
    },
  },
]
