import { Spin } from 'antd'
import type { FileViewerSlotProps } from '../../types'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { RawCodeView } from '../shared/RawCodeView'
import { DelimitedTable } from './DelimitedTable'

function delimitedBody(delimiter: string) {
  return function DelimitedBody({ file }: FileViewerSlotProps) {
    const content = useFileTextContent(file)
    const mode = useFileViewMode(file.id)
    if (content === null) {
      return <div className="flex items-center justify-center h-full"><Spin /></div>
    }
    if (mode === 'raw') return <RawCodeView text={content} />
    return <DelimitedTable text={content} delimiter={delimiter} />
  }
}

export const CsvBody = delimitedBody(',')
export const TsvBody = delimitedBody('\t')
export { XlsxBody } from './XlsxBody'
