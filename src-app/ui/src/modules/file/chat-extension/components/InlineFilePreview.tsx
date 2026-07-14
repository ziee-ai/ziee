import { ChevronRight, ChevronDown, File, PanelRight } from 'lucide-react'
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { Button, Tooltip } from '@ziee/kit'
import { cn } from '@/lib/utils'
import { Stores } from '@/core/stores'
import type { File as FileEntity } from '@/api-client/types'
import type { FileViewerEntry, FileViewerSlotProps, InlineFileSource } from '@/modules/file/types/viewer'
import { isInlineCapable } from '@/modules/file/viewers/shared/source'
import { DownloadButton } from '@/modules/file/viewers/shared/chrome'
// NOTE: this file (a chat-EXTENSION of the file module) intentionally reaches
// into chat-module UI state — the lifted per-conversation view store + the
// in-place-anchor hook. This is chat coupling by design (an inline file preview
// lives inside the virtualized chat message list); it differs from the earlier
// in-file note declining to read chat-STREAMING state, which remains avoided.
import {
  useMessageViewStateStore,
  type MessageViewFullState,
} from '@/modules/chat/core/stores/MessageViewState.store'
import { DEFAULT_INLINE_FILE_STATE } from '@/modules/chat/core/stores/messageViewState.helpers'
import {
  INLINE_FILE_MIN_PX,
  clampReservedPx,
  maxReservedPx,
  resolveBodyHeightPx,
} from '@/modules/file/chat-extension/components/inlineFileHeight'
import { useInPlaceAnchor } from '@/modules/chat/core/utils/useInPlaceAnchor'

interface InlineFilePreviewProps {
  /** Viewer matched by `getViewer(name, mimeType)`. `undefined` when no
   *  viewer claims this MIME/ext — falls back to a header-only file card. */
  viewer: FileViewerEntry | undefined
  source: InlineFileSource
  /** Resolved File entity when this link is a backend-owned artifact. When
   *  present, the body renders through the authenticated `{file}` path (same
   *  as the right-side panel) and the header gains an "Open in side panel"
   *  button. Absent for external MCP links (URL-based `{source}` path). */
  file?: FileEntity
}

function formatFileSize(bytes: number | undefined): string {
  // A missing / malformed size (undefined, null, NaN) must never render as
  // "NaN GB" — show nothing instead.
  if (bytes === undefined || !Number.isFinite(bytes)) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

const KEY_STEP_PX = 24
const viewportH = () =>
  typeof window === 'undefined' ? 800 : window.innerHeight

/**
 * Collapsible wrapper around a single tool-result file.
 *
 * - **Header** (always visible): viewer icon + filename + label + size +
 *   the viewer's `headerActions` + side-panel button + collapse chevron.
 * - **Body** (when expanded AND viewer is inline-capable for this MIME): the
 *   viewer's `body` in a FIXED-HEIGHT, internally-scrolling box. The height is
 *   definite (not content-driven) so the virtualized message row's height stops
 *   changing after mount — image decode / table parse / Shiki highlight all
 *   settle inside the box (message-scroll-stability ITEM-2). Until the preview
 *   has been SEEN once, a same-height SKELETON stands in (its own testid, so the
 *   heavy viewer body is still lazily mounted only in view — preserving the
 *   existing lazy-mount perf contract), making the seen→body swap zero-delta.
 *
 * All ephemeral per-preview state (collapsed, seen, resized height) is LIFTED
 * into the per-conversation `MessageViewState` store keyed by the resource_link
 * URI (ITEM-5), so it survives the virtualizer unmounting/remounting this row.
 * A bottom drag/keyboard-resize handle (ITEM-3) lets the user grow/shrink one
 * preview; the chosen height persists there.
 */
export function InlineFilePreview({ viewer, source, file }: InlineFilePreviewProps) {
  const key = source.url
  const rootRef = useRef<HTMLDivElement>(null)
  const bodyId = useId()
  const anchorBeforeChange = useInPlaceAnchor(rootRef)

  // Scoped subscription: read only THIS preview's entry, not the whole `files`
  // map. `markFileSeen` fires on scroll as previews enter view; a whole-map read
  // would re-render every mounted preview on each scroll-in (a scroll-driven
  // re-render storm that undercuts smoothness). immer's structural sharing keeps
  // `files[key]` identity stable when a DIFFERENT key mutates, so this selector
  // re-renders only when this preview's own state changes.
  const view =
    useMessageViewStateStore(
      (s: MessageViewFullState) => s.files[key],
    ) ?? DEFAULT_INLINE_FILE_STATE
  const collapsed = view.collapsed

  // Viewport-gate the body ONCE per conversation session: mark `seen` when it
  // scrolls within ~800px of the viewport, then remember it in the store so any
  // later remount mounts the body immediately at the SAME fixed height — no
  // re-fetch, no re-lazy-mount height churn (ITEM-5, DEC-10). First load starts
  // `seen:false`, so off-screen bodies still defer their fetch.
  useEffect(() => {
    if (view.seen) return
    if (typeof IntersectionObserver === 'undefined') {
      Stores.MessageViewState.markFileSeen(key)
      return
    }
    const el = rootRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      entries => {
        if (entries.some(e => e.isIntersecting)) {
          Stores.MessageViewState.markFileSeen(key)
          observer.disconnect()
        }
      },
      { rootMargin: '800px 0px' },
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [view.seen, key])

  // Prefer the resolved File's metadata (authoritative) over the link's.
  const displayName = file?.filename ?? source.name
  const displayMime = file?.mime_type ?? source.mimeType
  const displaySize = file?.file_size ?? source.size

  const canInline = isInlineCapable(viewer, displayName, displayMime ?? undefined)
  const Body = canInline ? viewer?.body : undefined
  const HeaderActions = canInline ? viewer?.headerActions : undefined
  const Icon = viewer?.icon ?? <File />
  const label = viewer?.label
  const inlineFill = viewer?.inlineFill ?? false

  // Fixed body height: the persisted resized px (during a drag, the live local
  // value) or the per-viewer default. Skeleton + body use the SAME value so the
  // seen→body swap is zero-delta.
  const [dragHeight, setDragHeight] = useState<number | null>(null)
  const bodyHeightPx =
    dragHeight ?? resolveBodyHeightPx(inlineFill, view.heightPx, viewportH())

  const hasBody = canInline && Body !== undefined
  const showBodyRegion = hasBody && !collapsed
  const slotProps: FileViewerSlotProps = file ? { file } : { source }

  const setCollapsed = (next: boolean) => {
    anchorBeforeChange()
    Stores.MessageViewState.setFileCollapsed(key, next)
  }

  const handleOpenInPanel = () => {
    if (!file) return
    Stores.Chat.displayInRightPanel({
      id: file.id,
      title: file.filename,
      type: 'file',
      data: { fileId: file.id, version: source.version },
    })
  }

  // ── Drag/keyboard resize (ITEM-3). Bottom-edge drag grows the box downward so
  // the row top never moves (in-place). The live drag height is held in local
  // state + a ref (so the commit reads it WITHOUT a side-effecting setState
  // updater); committed to the store on release/cancel so it persists across
  // remount. `onLostPointerCapture`/`onPointerCancel` guarantee the drag always
  // ends (no stuck drag if the browser cancels the gesture). Keyboard-resizable
  // for a11y (DEC-6).
  const dragStart = useRef<{ y: number; h: number } | null>(null)
  const dragHeightRef = useRef<number | null>(null)
  const setDrag = (px: number | null) => {
    dragHeightRef.current = px
    setDragHeight(px)
  }
  const onHandlePointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault()
    // setPointerCapture can throw (NotFoundError) if the pointer isn't active
    // (e.g. a synthetic/dispatched event) — the drag still works via the
    // element's own move handler, so never let a capture failure abort it.
    try {
      ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
    } catch {
      /* capture unavailable — proceed without it */
    }
    dragStart.current = { y: e.clientY, h: bodyHeightPx }
    setDrag(bodyHeightPx)
  }
  const onHandlePointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const start = dragStart.current
    if (!start) return
    setDrag(clampReservedPx(start.h + (e.clientY - start.y), viewportH()))
  }
  const endDrag = useCallback(() => {
    if (dragStart.current == null) return
    dragStart.current = null
    const px = dragHeightRef.current
    if (px != null) Stores.MessageViewState.setFileHeight(key, px)
    setDrag(null)
  }, [key])
  const onHandlePointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    // Guard symmetrically with setPointerCapture: releasePointerCapture throws
    // NotFoundError on spec-conformant engines (WebKit/Firefox) when no capture
    // is active (e.g. a synthetic/dispatched pointer). A throw here must NOT
    // abort endDrag() → the height commit.
    try {
      ;(e.target as HTMLElement).releasePointerCapture?.(e.pointerId)
    } catch {
      /* no active capture — ignore */
    }
    endDrag()
  }
  const onHandleKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    let next: number | null = null
    if (e.key === 'ArrowUp') next = bodyHeightPx - KEY_STEP_PX
    else if (e.key === 'ArrowDown') next = bodyHeightPx + KEY_STEP_PX
    else if (e.key === 'Home') next = 0 // clamp → min
    else if (e.key === 'End') next = Number.MAX_SAFE_INTEGER // clamp → max
    if (next == null) return
    e.preventDefault()
    anchorBeforeChange()
    Stores.MessageViewState.setFileHeight(key, clampReservedPx(next, viewportH()))
  }

  return (
    <div
      ref={rootRef}
      data-testid="inline-file-preview"
      data-file-uri={source.url}
      data-file-id={file?.id}
      className="flex flex-col rounded-md overflow-hidden border border-border bg-card"
    >
      {/* Header row (flex-wrap for narrow inline widths). */}
      <div
        className="flex flex-wrap items-center gap-x-2 gap-y-1 px-3 py-2 bg-muted/60"
        style={{
          borderBottom: showBodyRegion ? '1px solid var(--border)' : 'none',
        }}
      >
        <div className="flex items-center gap-2 min-w-0 flex-1 overflow-hidden">
          <span
            className="flex-shrink-0 inline-flex items-center justify-center text-muted-foreground"
            style={{ width: 20, height: 20 }}
            data-testid="inline-file-preview-icon"
          >
            {Icon}
          </span>
          <span
            className="font-medium truncate text-foreground"
            title={displayName}
          >
            {displayName}
          </span>
          <span className="text-xs flex-shrink-0 whitespace-nowrap text-muted-foreground">
            {label ? <>· {label}</> : null}
            {displaySize !== undefined ? <> · {formatFileSize(displaySize)}</> : null}
          </span>
        </div>
        <div className="flex items-center gap-0.5 flex-shrink-0 ms-auto">
          {HeaderActions ? <HeaderActions {...slotProps} /> : null}
          {file ? <DownloadButton file={file} /> : null}
          {file ? (
            <Tooltip content="Open in side panel">
              <Button
                variant="ghost"
                size="default"
                icon={<PanelRight />}
                onClick={handleOpenInPanel}
                aria-label="Open file in side panel"
                data-testid="inline-file-preview-open-panel"
              />
            </Tooltip>
          ) : null}
          {hasBody && (
            <Button
              variant="ghost"
              size="default"
              aria-label={collapsed ? 'Expand file preview' : 'Collapse file preview'}
              aria-expanded={!collapsed}
              // Only reference the region while it is actually mounted (expanded);
              // when collapsed the body/skeleton is unmounted, so a dangling
              // aria-controls IDREF would be invalid (re-audit low finding).
              aria-controls={collapsed ? undefined : bodyId}
              icon={collapsed ? <ChevronRight /> : <ChevronDown />}
              onClick={() => setCollapsed(!collapsed)}
              data-testid="inline-file-preview-chevron"
            />
          )}
        </div>
      </div>

      {/* Body region — FIXED height + internal scroll (ITEM-2). The heavy viewer
          body mounts ONLY once seen; before that a same-height skeleton stands
          in (distinct testid) so off-screen bodies stay lazily mounted AND the
          seen→body swap is zero-delta to the virtualizer. */}
      {showBodyRegion && (
        <>
          {view.seen && Body ? (
            <div
              id={bodyId}
              className="overflow-auto"
              style={{ height: bodyHeightPx }}
              data-testid="inline-file-preview-body"
              data-body-height={bodyHeightPx}
            >
              <Body {...slotProps} />
            </div>
          ) : (
            <div
              id={bodyId}
              className="overflow-hidden"
              style={{ height: bodyHeightPx }}
              data-testid="inline-file-preview-skeleton"
              data-body-height={bodyHeightPx}
            >
              {/* Decorative loading placeholder — hidden from AT; the region
                  itself stays referenceable by the resize separator. */}
              <div
                className="h-full w-full animate-pulse bg-muted/40"
                aria-hidden="true"
              />
            </div>
          )}
          {/* Bottom drag/keyboard resize handle (ITEM-3). */}
          <div
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize file preview"
            aria-controls={bodyId}
            aria-valuenow={Math.round(bodyHeightPx)}
            aria-valuemin={INLINE_FILE_MIN_PX}
            aria-valuemax={maxReservedPx(viewportH())}
            tabIndex={0}
            data-testid="inline-file-preview-resize"
            onPointerDown={onHandlePointerDown}
            onPointerMove={onHandlePointerMove}
            onPointerUp={onHandlePointerUp}
            onPointerCancel={endDrag}
            onLostPointerCapture={endDrag}
            onKeyDown={onHandleKeyDown}
            className={cn(
              'h-2 w-full shrink-0 cursor-row-resize touch-none select-none',
              'bg-muted hover:bg-accent focus-visible:outline-none',
              'focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset',
            )}
          >
            {/* Grip cue — muted-foreground for a perceivable 3:1 boundary. */}
            <div className="mx-auto mt-[3px] h-0.5 w-8 rounded-full bg-muted-foreground/50" />
          </div>
        </>
      )}
    </div>
  )
}
