import { Spin } from 'antd'
import { Stores } from '@/core/stores'
import { RawCodeView } from '../shared/RawCodeView'
import type { FileViewRendererProps } from '../../types'

export function TextViewer({ file }: FileViewRendererProps) {
  const content = Stores.Chat.FileStore.getFileTextContent(file.id, file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return <RawCodeView text={content} />
}
