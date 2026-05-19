import type { FileViewerModule } from '../../types'
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
    },
  },
]
