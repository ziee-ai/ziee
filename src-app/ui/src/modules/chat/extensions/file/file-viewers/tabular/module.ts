import type { FileViewerModule } from '../../types'
import { CsvViewer, TsvViewer } from './DelimitedViewer'
import { XlsxViewer } from './XlsxViewer'

function ext(filename: string) {
  return filename.split('.').pop()?.toLowerCase() ?? ''
}

function isXlsx(filename: string, mimeType?: string): boolean {
  const e = ext(filename)
  if (e === 'xlsx' || e === 'xls' || e === 'ods') return true
  if (mimeType === 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet') return true
  if (mimeType === 'application/vnd.ms-excel') return true
  return false
}

export const viewers: FileViewerModule[] = [
  {
    canHandle: (filename, mimeType) => ext(filename) === 'csv' || mimeType === 'text/csv',
    entry: {
      render: props => CsvViewer(props),
      label: 'CSV',
      compilable: true,
      canCopy: true,
    },
  },
  {
    canHandle: (filename, mimeType) => ext(filename) === 'tsv' || mimeType === 'text/tab-separated-values',
    entry: {
      render: props => TsvViewer(props),
      label: 'TSV',
      compilable: true,
      canCopy: true,
    },
  },
  {
    canHandle: isXlsx,
    entry: {
      render: props => XlsxViewer(props),
      label: 'Spreadsheet',
      compilable: false,
      canCopy: false,
    },
  },
]
