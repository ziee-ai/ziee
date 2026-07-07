import { FileImage } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Spin } from '@/components/ui'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'
import { getSource } from '../shared/source'
import { DEFAULT_IMAGE_VIEW, clampTranslate } from './zoom'

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
          <FileImage className="text-2xl" />
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

  // ── Right-panel context (existing behaviour + zoom/pan).
  return <ImagePanelBody file={file} />
}

/** Right-panel image body: authenticated thumbnail + zoom/fit + pan-when-zoomed.
 *  Split into its own component so the hooks below aren't conditional on the
 *  inline branch above (rules-of-hooks). */
function ImagePanelBody({ file }: { file: NonNullable<ReturnType<typeof getSource>['file']> }) {
  const fileId = file.id
  const filename = file.filename
  // Subscribe to the thumbnailUrls Map by accessing it directly during
  // render — calling the `getThumbnailUrl()` action instead would only
  // subscribe to the function reference, not the Map's contents, so we
  // wouldn't re-render when loadThumbnail finishes.
  const thumbnailUrls = Stores.File.thumbnailUrls
  const thumbnailUrl = thumbnailUrls.get(fileId) ?? null
  // Trigger background load on first call (idempotent — guarded by
  // thumbnailLoadingSet inside the store).
  if (thumbnailUrl === null) Stores.File.getThumbnailUrl(fileId, file)

  const view = Stores.File.imageViewStates.get(fileId) ?? DEFAULT_IMAGE_VIEW
  const containerRef = useRef<HTMLDivElement>(null)
  const imgRef = useRef<HTMLImageElement>(null)
  const [translate, setTranslate] = useState({ x: 0, y: 0 })
  const drag = useRef<{ x: number; y: number; tx: number; ty: number } | null>(null)

  // Returns the current per-axis overflow (scaled content minus container) so
  // the pan can be clamped to the pannable range.
  const overflow = () => {
    const c = containerRef.current
    const img = imgRef.current
    if (!c || !img) return { x: 0, y: 0 }
    return {
      x: img.naturalWidth * view.scale - c.clientWidth,
      y: img.naturalHeight * view.scale - c.clientHeight,
    }
  }

  // Reset pan whenever we return to fit (or the file changes) — a fit image has
  // nothing to pan, and a stale translate would offset it.
  useEffect(() => {
    if (view.mode === 'fit') setTranslate({ x: 0, y: 0 })
  }, [view.mode, view.scale, fileId])

  if (!thumbnailUrl) {
    return (
      <div className="flex items-center justify-center py-8">
        <Spin label="Loading" />
      </div>
    )
  }

  // Fit mode — the original object-contain render (no transform, no pan).
  if (view.mode === 'fit') {
    return (
      <div
        className="flex items-center justify-center h-full w-full p-4 overflow-hidden"
        data-testid="image-viewer-body"
        data-view-mode="fit"
      >
        <img
          src={thumbnailUrl}
          alt={filename}
          loading="lazy"
          decoding="async"
          className="max-w-full max-h-full object-contain"
        />
      </div>
    )
  }

  // Actual / zoomed — natural-size image transformed by scale, pannable by drag.
  const onPointerDown = (e: React.PointerEvent) => {
    const o = overflow()
    if (o.x <= 0 && o.y <= 0) return // nothing to pan
    drag.current = { x: e.clientX, y: e.clientY, tx: translate.x, ty: translate.y }
    ;(e.target as HTMLElement).setPointerCapture?.(e.pointerId)
  }
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    const o = overflow()
    setTranslate(
      clampTranslate(d.tx + (e.clientX - d.x), d.ty + (e.clientY - d.y), o.x, o.y),
    )
  }
  const endDrag = (e: React.PointerEvent) => {
    drag.current = null
    ;(e.target as HTMLElement).releasePointerCapture?.(e.pointerId)
  }

  return (
    <div
      ref={containerRef}
      className="flex items-center justify-center h-full w-full overflow-hidden touch-none select-none"
      style={{ cursor: drag.current ? 'grabbing' : 'grab' }}
      data-testid="image-viewer-body"
      data-view-mode="actual"
      data-scale={view.scale}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerLeave={endDrag}
    >
      <img
        ref={imgRef}
        src={thumbnailUrl}
        alt={filename}
        decoding="async"
        draggable={false}
        style={{
          transform: `translate(${translate.x}px, ${translate.y}px) scale(${view.scale})`,
          transformOrigin: 'center',
          maxWidth: 'none',
        }}
      />
    </div>
  )
}
