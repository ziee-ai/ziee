import { lazy } from 'react'
import { FileText } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'

const PdfBody = lazy(() => import('./body').then(m => ({ default: m.PdfBody })))
const PdfHeader = lazy(() => import('./header').then(m => ({ default: m.PdfHeader })))
const PdfJsBody = lazy(() => import('./pdfjs-body').then(m => ({ default: m.PdfJsBody })))

// PPTX intentionally not registered. Backend can't convert PPTX to
// PDF: pandoc doesn't read PowerPoint, and `office2pdf` (the only
// viable pure-Rust converter found) is currently published broken
// against the latest quick-xml. Reach back here when an upstream
// converter is available.

export const viewers: FileViewerModule[] = [
  {
    // Real PDFs render client-side via PDF.js's PDFViewer component
    // (PdfJsBody) — native page-nav / zoom / find / text-selection, no
    // 50-page cap. The original bytes come from `/files/{id}/raw`. The
    // toolbar lives inside PdfJsBody (header keeps only Download), because
    // the header/body are independent components (see DEC-6/DEC-11).
    supportedTypes: [{ mime: 'application/pdf' }, { ext: 'pdf' }],
    entry: {
      body: PdfJsBody,
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
