import { lazy } from 'react'
import { FileCode, FileImage } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'

const WebBody = lazy(() => import('./body').then(m => ({ default: m.WebBody })))
const WebHeader = lazy(() => import('./header').then(m => ({ default: m.WebHeader })))

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
      icon: <FileCode />,
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
      icon: <FileImage />,
    },
  },
]
