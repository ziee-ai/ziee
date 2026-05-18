import { Spin } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewRendererProps } from '../../types'

export function WebViewer({ file }: FileViewRendererProps) {
  const { fileTextContents } = Stores.Chat.FileStore
  const content = fileTextContents.get(file.id) ?? null
  if (content === null) Stores.Chat.FileStore.getFileTextContent(file.id, file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return (
    <iframe
      sandbox="allow-scripts"
      srcDoc={content}
      style={{ width: '100%', height: '100%', border: 'none', minHeight: 400 }}
      title="preview"
    />
  )
}
