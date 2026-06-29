import { Spin } from '@/components/ui'
import type { FileViewerSlotProps } from '../../types/viewer'
import { useFileTextContent, useFileViewMode } from '../shared/hooks'
import { useResourceLinkContent } from '../../hooks/useResourceLinkContent'
import { RawCodeView } from '../shared/RawCodeView'
import { DelimitedTable } from './DelimitedTable'
import { getSource } from '../shared/source'

function delimitedBody(delimiter: string) {
  return function DelimitedBody(props: FileViewerSlotProps) {
    const { file, url } = getSource(props)
    const rightPanelContent = useFileTextContent(file, !file)
    const inlineContent = useResourceLinkContent(url, !!file)
    const content = file ? rightPanelContent : inlineContent
    const mode = useFileViewMode(file?.id ?? '')

    if (content === '__error__') {
      return (
        <div className="flex items-center justify-center h-full text-sm opacity-70 p-4">
          Failed to load file content.
        </div>
      )
    }
    if (content === null) {
      return <div className="flex items-center justify-center h-full"><Spin label="Loading" /></div>
    }
    if (file && mode === 'raw') return <RawCodeView text={content} />
    return <DelimitedTable text={content} delimiter={delimiter} />
  }
}

export const CsvBody = delimitedBody(',')
export const TsvBody = delimitedBody('\t')
export { XlsxBody } from './XlsxBody'
