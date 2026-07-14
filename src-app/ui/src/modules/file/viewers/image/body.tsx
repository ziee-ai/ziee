import { FileImage } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Spin } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
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
  const [dragging, setDragging] = useState(false)
  // Drag session — dims are measured ONCE at pointerdown (avoids a forced reflow
  // on every move); `latest` holds the running translate written directly to the
  // img style during the drag, then committed to React state on pointerup.
  const drag = useRef<
    | { x: number; y: number; tx: number; ty: number; ox: number; oy: number; latest: { x: number; y: number } }
    | null
  >(null)

  // Current per-axis overflow (scaled content minus container). Reads layout, so
  // call it sparingly (drag start / keyboard step / scale-change re-clamp).
  const overflow = () => {
    const c = containerRef.current
    const img = imgRef.current
    if (!c || !img) return { x: 0, y: 0 }
    return {
      x: img.naturalWidth * view.scale - c.clientWidth,
      y: img.naturalHeight * view.scale - c.clientHeight,
    }
  }

  // rAF handle coalescing pointermove → one setState per frame.
  const rafRef = useRef(0)

  // Re-clamp pan whenever the mode/scale changes: fit has nothing to pan (→ 0);
  // a zoom-OUT shrinks the overflow so a prior translate must be pulled back
  // inside the new bounds (else the image is stranded partly off-screen).
  useEffect(() => {
    if (view.mode === 'fit') {
      setTranslate({ x: 0, y: 0 })
      return
    }
    const o = overflow()
    setTranslate(prev => clampTranslate(prev.x, prev.y, o.x, o.y))
    // fileId included so a reused panel resets for a different file.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.mode, view.scale, fileId])

  // Re-clamp on container resize too (drawer/window resize shrinks the overflow
  // and would otherwise strand a panned image off-screen). Actual mode only.
  useEffect(() => {
    if (view.mode === 'fit') return
    const c = containerRef.current
    if (!c || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => {
      const o = overflow()
      setTranslate(prev => clampTranslate(prev.x, prev.y, o.x, o.y))
    })
    ro.observe(c)
    return () => ro.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.mode, view.scale])

  // Cancel any pending rAF on unmount.
  useEffect(() => () => cancelAnimationFrame(rafRef.current), [])

  // Trackpad pinch-zoom. A native NON-passive listener is required so we can
  // preventDefault (React's onWheel is passive). Only the ZOOM gesture is
  // intercepted — a trackpad pinch emits a `wheel` with ctrlKey set (as does
  // ctrl/⌘+wheel on a mouse); a plain wheel-scroll is left alone. Re-attaches on
  // a mode flip since fit and actual render different root elements. `exp` keeps
  // a pinch's small deltas smooth; zoomImage clamps to [0.1, 8] and switches the
  // view to 'actual'.
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return
      e.preventDefault()
      Stores.File.zoomImage(fileId, Math.exp(-e.deltaY * 0.0015))
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return () => el.removeEventListener('wheel', onWheel)
  }, [fileId, view.mode])

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
        ref={containerRef}
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

  // Actual / zoomed — natural-size image transformed by scale, pannable by drag
  // AND by keyboard (arrow keys) for accessibility.
  const onPointerDown = (e: React.PointerEvent) => {
    const o = overflow()
    if (o.x <= 0 && o.y <= 0) return // nothing to pan
    drag.current = {
      x: e.clientX,
      y: e.clientY,
      tx: translate.x,
      ty: translate.y,
      ox: o.x,
      oy: o.y,
      latest: translate,
    }
    setDragging(true)
    ;(e.target as HTMLElement).setPointerCapture?.(e.pointerId)
  }
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    // Dims measured once at pointerdown (no per-move reflow). Commit through
    // React state, but COALESCED to one setState per animation frame so a fast
    // pointer stream doesn't re-render per event — and React stays the source of
    // truth for the transform (a direct style write would be reverted by any
    // interleaved re-render, snapping the image back mid-drag).
    d.latest = clampTranslate(d.tx + (e.clientX - d.x), d.ty + (e.clientY - d.y), d.ox, d.oy)
    if (rafRef.current) return
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0
      if (drag.current) setTranslate(drag.current.latest)
    })
  }
  const endDrag = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return // not an active drag (e.g. pointerleave with no drag) — no-op
    drag.current = null
    setDragging(false)
    cancelAnimationFrame(rafRef.current)
    rafRef.current = 0
    setTranslate(d.latest) // commit the final position
    const el = e.target as HTMLElement
    // Only release if this element still holds the capture (a prior pointerup may
    // already have released it — releasing a stale pointerId throws).
    if (el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId)
  }
  const onKeyDown = (e: React.KeyboardEvent) => {
    const STEP = 40
    const delta: Record<string, [number, number]> = {
      ArrowLeft: [STEP, 0],
      ArrowRight: [-STEP, 0],
      ArrowUp: [0, STEP],
      ArrowDown: [0, -STEP],
    }
    const d = delta[e.key]
    if (!d) return
    // Check overflow BEFORE preventing default — if there's nothing to pan, let
    // the key do its normal thing instead of swallowing it (no scroll trap).
    const o = overflow()
    if (o.x <= 0 && o.y <= 0) return
    e.preventDefault()
    setTranslate(prev => clampTranslate(prev.x + d[0], prev.y + d[1], o.x, o.y))
  }

  return (
    <div
      ref={containerRef}
      className="flex items-center justify-center h-full w-full overflow-hidden touch-none select-none outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
      style={{ cursor: dragging ? 'grabbing' : 'grab' }}
      data-testid="image-viewer-body"
      data-view-mode="actual"
      data-scale={view.scale}
      // Focusable pannable region — role=group (not img: img would make the
      // subtree presentational and hide that it's an operable widget). The
      // roledescription + label announce the arrow-key affordance.
      tabIndex={0}
      role="group"
      aria-roledescription="Pannable image"
      aria-label={`${filename} — zoomed; use arrow keys to pan`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerLeave={endDrag}
      onKeyDown={onKeyDown}
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
