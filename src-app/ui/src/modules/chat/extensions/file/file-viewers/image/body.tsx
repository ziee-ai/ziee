import { Spin } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types'

export function ImageBody({ file }: FileViewerSlotProps) {
  // Subscribe to the thumbnailUrls Map by accessing it directly during
  // render — calling the `getThumbnailUrl()` action instead would only
  // subscribe to the function reference, not the Map's contents, so we
  // wouldn't re-render when loadThumbnail finishes.
  const thumbnailUrls = Stores.Chat.FileStore.thumbnailUrls
  const thumbnailUrl = thumbnailUrls.get(file.id) ?? null
  // Trigger background load on first call (idempotent — guarded by
  // thumbnailLoadingSet inside the store).
  if (thumbnailUrl === null) Stores.Chat.FileStore.getThumbnailUrl(file.id, file)

  if (!thumbnailUrl) {
    return <div className="flex items-center justify-center py-8"><Spin /></div>
  }
  return (
    <div className="flex items-center justify-center h-full p-4">
      <img src={thumbnailUrl} alt={file.filename} className="max-w-full object-contain" />
    </div>
  )
}
