import { FileText } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'
import { PdfBody } from './body'
import { PdfHeader } from './header'

// PPTX intentionally not registered. Backend can't convert PPTX to
// PDF: pandoc doesn't read PowerPoint, and `office2pdf` (the only
// viable pure-Rust converter found) is currently published broken
// against the latest quick-xml. Reach back here when an upstream
// converter is available.

export const viewers: FileViewerModule[] = [
  {
    supportedTypes: [{ mime: 'application/pdf' }, { ext: 'pdf' }],
    entry: {
      body: PdfBody,
      headerActions: PdfHeader,
      label: 'PDF',
      icon: <FileText />,
    },
  },
  {
    // DOCX / DOC / RTF / ODT — backend OfficeProcessor pipes these
    // through Pandoc → typst → PDF → PdfProcessor.generate_images(),
    // which emits one preview-image per rendered page. From the
    // frontend's perspective the file looks identical to a PDF;
    // PdfBody handles both.
    supportedTypes: [
      { mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' },
      { mime: 'application/msword' },
      { mime: 'application/rtf' },
      { mime: 'text/rtf' },
      { mime: 'application/vnd.oasis.opendocument.text' },
      { ext: 'docx' },
      { ext: 'doc' },
      { ext: 'rtf' },
      { ext: 'odt' },
    ],
    entry: {
      body: PdfBody,
      headerActions: PdfHeader,
      label: 'Document',
      icon: <FileText />,
    },
  },
]
