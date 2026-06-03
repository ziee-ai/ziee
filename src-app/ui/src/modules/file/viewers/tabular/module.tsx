import type { FileViewerModule } from '../../types/viewer'
import { FileExcelOutlined } from '@ant-design/icons'
import { CsvBody, TsvBody, XlsxBody } from './body'
import { DelimitedHeader, XlsxHeader } from './header'

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
      icon: <FileExcelOutlined />,
      // Text-based delimited format; renders inline as a DelimitedTable.
      inline: true,
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
      icon: <FileExcelOutlined />,
      inline: true,
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
      icon: <FileExcelOutlined />,
      // No `inline` — XLSX needs binary parsing + the xlsx library
      // (heavy dynamic import) + FileStore.fileBinaryContents. The
      // inline-context path doesn't have any of that. Defer.
    },
  },
]
