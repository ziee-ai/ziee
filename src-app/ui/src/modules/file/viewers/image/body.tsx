import { useState } from 'react'
import { Spin } from 'antd'
import { FileImageOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'
import { getSource } from '../shared/source'

export function ImageBody(props: FileViewerSlotProps) {
  const { file, url, name } = getSource(props)
  // Tracks load failure for the inline (external-MCP) <img> path so we can
  // show a visible placeholder instead of silently falling back to alt-text.
  const [errored, setErrored] = useState(false)

  // ── Inline-in-chat context: no FileEntity, no thumbnail cache.
  // Render the image directly from the resource_link URL (external MCP).
  // Backend-owned artifacts arrive with a FileEntity and take the
  // authenticated thumbnail path below instead. The collapse wrapper handles
  // size clamping; object-contain scales a wide image inside it.
  if (!file) {
    if (errored) {
      return (
        <div
          className="flex flex-col items-center justify-center gap-1 p-6 text-sm opacity-60"
          data-testid="inline-file-preview-image-error"
        >
          <FileImageOutlined style={{ fontSize: 24 }} />
          <span>Couldn't load image</span>
        </div>
      )
    }
    return (
      <div className="flex items-center justify-center p-4">
        <img
          src={url}
          alt={name}
          loading="lazy"
          decoding="async"
          className="max-w-full max-h-[400px] object-contain"
          onError={() => setErrored(true)}
        />
      </div>
    )
  }

  // ── Right-panel context (existing behaviour).
  // Subscribe to the thumbnailUrls Map by accessing it directly during
  // render — calling the `getThumbnailUrl()` action instead would only
  // subscribe to the function reference, not the Map's contents, so we
  // wouldn't re-render when loadThumbnail finishes.
  const thumbnailUrls = Stores.File.thumbnailUrls
  const thumbnailUrl = thumbnailUrls.get(file.id) ?? null
  // Trigger background load on first call (idempotent — guarded by
  // thumbnailLoadingSet inside the store).
  if (thumbnailUrl === null) Stores.File.getThumbnailUrl(file.id, file)

  if (!thumbnailUrl) {
    return <div className="flex items-center justify-center py-8"><Spin /></div>
  }
  return (
    <div className="flex items-center justify-center h-full p-4">
      <img src={thumbnailUrl} alt={file.filename} loading="lazy" decoding="async" className="max-w-full object-contain" />
    </div>
  )
}
