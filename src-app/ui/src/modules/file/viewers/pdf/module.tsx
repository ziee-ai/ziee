import type { FileViewerModule } from '../../types/viewer'
import { FilePdfOutlined } from '@ant-design/icons'
import { PdfBody } from './body'
import { PdfHeader } from './header'

export const viewers: FileViewerModule[] = [
  {
    supportedTypes: [{ mime: 'application/pdf' }, { ext: 'pdf' }],
    entry: {
      body: PdfBody,
      headerActions: PdfHeader,
      label: 'PDF',
      icon: <FilePdfOutlined />,
    },
  },
]
