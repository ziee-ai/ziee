import { lazy } from 'react'
import { Image } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'

const ImageBody = lazy(() => import('./body').then(m => ({ default: m.ImageBody })))
const ImageHeader = lazy(() => import('./header').then(m => ({ default: m.ImageHeader })))

export const viewers: FileViewerModule[] = [
  {
    // Wildcard match for any image MIME — `web/` viewer overrides for SVG
    // with an exact priority-0 rule. Higher priority value = less specific
    // = loses on conflict.
    supportedTypes: [{ mime: 'image/*', priority: 10 }],
    entry: {
      body: ImageBody,
      headerActions: ImageHeader,
      label: 'Image',
      icon: <Image />,
      // Images are the headline inline-render use case (plots, heatmaps).
      // `image/*` wildcard covers png/jpeg/webp/gif/svg+xml — anything the
      // browser can <img>.
      inline: true,
    },
  },
]
