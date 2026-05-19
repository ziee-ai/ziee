import type { FileViewerModule } from '../../types'
import { PictureOutlined } from '@ant-design/icons'
import { ImageBody } from './body'
import { ImageHeader } from './header'

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
      icon: <PictureOutlined />,
    },
  },
]
