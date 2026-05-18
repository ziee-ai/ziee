import { Spin } from 'antd'
import { Streamdown } from 'streamdown'
import { Stores } from '@/core/stores'
import type { FileViewRendererProps } from '../../types'

export function MarkdownViewer({ file }: FileViewRendererProps) {
  const { fileTextContents } = Stores.Chat.FileStore
  const content = fileTextContents.get(file.id) ?? null
  if (content === null) Stores.Chat.FileStore.getFileTextContent(file.id, file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return (
    <div className="p-4 overflow-auto h-full">
      <Streamdown shikiTheme={['github-light', 'github-dark']}>{content}</Streamdown>
    </div>
  )
}
