import { Spin } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewRendererProps } from '../../types'

export function ImageViewer({ file }: FileViewRendererProps) {
  const thumbnailUrl = Stores.Chat.FileStore.getThumbnailUrl(file.id, file)
  if (!thumbnailUrl) {
    return <div className="flex items-center justify-center py-8"><Spin /></div>
  }
  return (
    <div className="flex items-center justify-center h-full p-4">
      <img src={thumbnailUrl} alt={file.filename} className="max-w-full object-contain" />
    </div>
  )
}
