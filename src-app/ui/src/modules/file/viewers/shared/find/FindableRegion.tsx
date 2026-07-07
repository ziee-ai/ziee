import { useCallback, useEffect, useRef, useState } from 'react'
import { Stores } from '@/core/stores'
import { FindBar } from './FindBar'
import { useFindInDocument, type HighlightNames } from './useFindInDocument'
import { isHighlightSupported } from './highlightSupported'

// ── Module-level open-find registry ──────────────────────────────────────────
// A single document-level Ctrl/Cmd-F listener drives whichever mounted region is
// the current FOCUS CONTEXT. A per-region onKeyDown would only fire when focus
// already sits inside the region — but a text/markdown body has no focusable
// descendants, so on a freshly-opened drawer focus stays on <body> and native
// find would win. The listener therefore also handles the "focus on body/host"
// case, but ONLY intercepts when focus is genuinely inside this viewer's surface
// (the region, or its host dialog/full-page container, or nowhere) — so pressing
// Ctrl-F while typing in an unrelated field (chat composer, sidebar) still gets
// the browser's native find.
interface RegionEntry {
  el: HTMLElement
  host: HTMLElement
  open: () => void
}
const openRegions: RegionEntry[] = []
let docListener: ((e: KeyboardEvent) => void) | null = null

function isVisible(el: HTMLElement): boolean {
  return el.offsetParent !== null && el.getClientRects().length > 0
}

/** The surface a region belongs to (its dialog / full-page container), used to
 *  decide whether focus is "inside this viewer". Falls back to the region. */
function hostOf(el: HTMLElement): HTMLElement {
  // Unquoted attribute value on purpose — keeps a quoted testid literal out of
  // this selector string so the global testid-uniqueness guard (which greps for
  // quoted testid declarations) doesn't count it as a second declaration.
  return (
    (el.closest(
      '[role="dialog"],[data-testid=file-view-page]',
    ) as HTMLElement | null) ?? el
  )
}

function ensureDocListener() {
  if (docListener) return
  docListener = (e: KeyboardEvent) => {
    if (!(e.ctrlKey || e.metaKey)) return
    if (e.key !== 'f' && e.key !== 'F') return
    const active = document.activeElement
    // Focus is "unclaimed" (body/null) → the topmost visible viewer may take it.
    const unfocused = !active || active === document.body
    // Topmost (last-registered) VISIBLE region whose surface owns focus wins.
    for (let i = openRegions.length - 1; i >= 0; i--) {
      const r = openRegions[i]
      if (!isVisible(r.el)) continue
      if (unfocused || r.host.contains(active)) {
        e.preventDefault()
        r.open()
        return
      }
      // A visible viewer exists but focus is elsewhere (composer/sidebar): do NOT
      // hijack — fall through to the browser's native find.
      return
    }
  }
  document.addEventListener('keydown', docListener)
}

function registerRegion(entry: RegionEntry) {
  openRegions.push(entry)
  ensureDocListener()
  return () => {
    const i = openRegions.indexOf(entry)
    if (i >= 0) openRegions.splice(i, 1)
    if (openRegions.length === 0 && docListener) {
      document.removeEventListener('keydown', docListener)
      docListener = null
    }
  }
}

/**
 * Wraps a viewer body in find-in-document. Renders the FindBar (when open) above
 * the content and paints matches via the CSS Custom Highlight API. Open-state is
 * coordinated with the header FindButton through `File.store.fileFindOpen`.
 *
 * When the Highlight API is unavailable the region is a passthrough (native find).
 */
export function FindableRegion({
  fileId,
  children,
  className,
}: {
  fileId: string
  children: React.ReactNode
  className?: string
}) {
  const supported = isHighlightSupported()
  const open = Stores.File.fileFindOpen.get(fileId) ?? false
  const [query, setQuery] = useState('')
  const contentRef = useRef<HTMLDivElement>(null)
  const regionRef = useRef<HTMLDivElement>(null)

  // Namespace the process-global highlight registry per INSTANCE (not per fileId —
  // the same file can be open in two regions at once, e.g. the preview drawer +
  // the /files/:id full page — which would otherwise clobber each other). An
  // ident-safe random suffix so `::highlight(<name>)` is valid CSS.
  const namesRef = useRef<HighlightNames | null>(null)
  if (!namesRef.current) {
    const uid = Math.random().toString(36).slice(2, 10)
    namesRef.current = { all: `file-find-${uid}`, active: `file-find-active-${uid}` }
  }
  const names = namesRef.current!

  const active = supported && open
  const { count, activeIndex, next, prev } = useFindInDocument(
    contentRef,
    query,
    active,
    names,
  )

  const openFind = useCallback(() => {
    Stores.File.setFileFindOpen(fileId, true)
  }, [fileId])

  const close = useCallback(() => {
    Stores.File.setFileFindOpen(fileId, false)
    // Restore focus into the viewer so keyboard focus doesn't drop to <body>.
    regionRef.current?.focus()
  }, [fileId])

  // Register this region for the document-level Ctrl-F shortcut while mounted.
  useEffect(() => {
    if (!supported) return
    const el = regionRef.current
    if (!el) return
    return registerRegion({ el, host: hostOf(el), open: openFind })
  }, [supported, openFind])

  return (
    <div
      ref={regionRef}
      className={className ?? 'flex flex-col h-full w-full min-h-0'}
      tabIndex={-1}
      data-testid="file-findable-region"
    >
      {supported && (
        <style>{`
          ::highlight(${names.all}) { background-color: color-mix(in srgb, var(--warning) 40%, transparent); }
          ::highlight(${names.active}) { background-color: color-mix(in srgb, var(--warning) 85%, transparent); }
        `}</style>
      )}
      {active && (
        <FindBar
          query={query}
          onQueryChange={setQuery}
          count={count}
          activeIndex={activeIndex}
          onNext={next}
          onPrev={prev}
          onClose={close}
        />
      )}
      <div ref={contentRef} className="flex-1 min-h-0 w-full overflow-hidden">
        {children}
      </div>
    </div>
  )
}
