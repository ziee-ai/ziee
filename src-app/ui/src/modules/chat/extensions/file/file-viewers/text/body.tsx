import { Spin } from 'antd'
import type { FileViewerSlotProps } from '../../types'
import { useFileTextContent } from '../shared/hooks'
import { RawCodeView } from '../shared/RawCodeView'

export function TextBody({ file }: FileViewerSlotProps) {
  const content = useFileTextContent(file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return <RawCodeView text={content} />
}
