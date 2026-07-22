import { lazy } from 'react'
import { FileSpreadsheet } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'

const CsvBody = lazy(() => import('./body').then(m => ({ default: m.CsvBody })))
const TsvBody = lazy(() => import('./body').then(m => ({ default: m.TsvBody })))
const XlsxBody = lazy(() => import('./body').then(m => ({ default: m.XlsxBody })))
const DelimitedHeader = lazy(() => import('./header').then(m => ({ default: m.DelimitedHeader })))
const XlsxHeader = lazy(() => import('./header').then(m => ({ default: m.XlsxHeader })))

export const viewers: FileViewerModule[] = [
  {
    supportedTypes: [
      { ext: 'csv' },
      { mime: 'text/csv' },
    ],
    entry: {
      body: CsvBody,
      headerActions: DelimitedHeader,
      label: 'CSV',
      icon: <FileSpreadsheet />,
      // Text-based delimited format; renders inline as a DelimitedTable.
      inline: true,
      // The DelimitedTable measures its container to size the virtual grid;
      // give it a definite inline height so that measurement doesn't loop.
      inlineFill: true,
    },
  },
  {
    supportedTypes: [
      { ext: 'tsv' },
      { mime: 'text/tab-separated-values' },
    ],
    entry: {
      body: TsvBody,
      headerActions: DelimitedHeader,
      label: 'TSV',
      icon: <FileSpreadsheet />,
      inline: true,
      inlineFill: true,
    },
  },
  {
    supportedTypes: [
      { ext: 'xlsx' },
      { ext: 'xls' },
      { ext: 'ods' },
      { mime: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' },
      { mime: 'application/vnd.ms-excel' },
    ],
    entry: {
      body: XlsxBody,
      headerActions: XlsxHeader,
      label: 'Spreadsheet',
      icon: <FileSpreadsheet />,
      // No `inline` — XLSX needs binary parsing + the xlsx library
      // (heavy dynamic import) + FileStore.fileBinaryContents. The
      // inline-context path doesn't have any of that. Defer.
    },
  },
]
