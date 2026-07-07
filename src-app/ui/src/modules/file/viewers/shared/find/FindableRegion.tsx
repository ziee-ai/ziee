import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Stores } from '@/core/stores'
import { FindBar } from './FindBar'
import { useFindInDocument, type HighlightNames } from './useFindInDocument'
import { isHighlightSupported } from './highlightSupported'

// ── Module-level open-find registry ──────────────────────────────────────────
// A single document-level Ctrl/Cmd-F listener drives whichever mounted region is
// topmost AND visible. A per-region onKeyDown would only fire when focus already
// sits inside the region — but a text/markdown body has no focusable descendants,
// so on a freshly-opened drawer focus stays on <body> and native find would win.
// This registry makes Ctrl-F open the in-app bar regardless of focus, while a
// hidden (inactive right-panel tab) region never steals the shortcut.
interface RegionEntry {
  el: HTMLElement
  open: () => void
}
const openRegions: RegionEntry[] = []
let docListener: ((e: KeyboardEvent) => void) | null = null

function isVisible(el: HTMLElement): boolean {
  return el.offsetParent !== null && el.getClientRects().length > 0
}

function ensureDocListener() {
  if (docListener) return
  docListener = (e: KeyboardEvent) => {
    if (!(e.ctrlKey || e.metaKey)) return
    if (e.key !== 'f' && e.key !== 'F') return
    // Topmost (last-registered) VISIBLE region wins.
    for (let i = openRegions.length - 1; i >= 0; i--) {
      if (isVisible(openRegions[i].el)) {
        e.preventDefault()
        openRegions[i].open()
        return
      }
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

  // Namespace the process-global highlight registry per file so two mounted
  // regions never clobber each other's highlights.
  const names: HighlightNames = useMemo(
    () => ({ all: `file-find-${fileId}`, active: `file-find-active-${fileId}` }),
    [fileId],
  )

  const active = supported && open
  const { count, activeIndex, next, prev } = useFindInDocument(
    contentRef,
    query,
    active,
    names,
  )

  const openFind = useCallback(() => {
    Stores.File.__state.setFileFindOpen(fileId, true)
  }, [fileId])

  const close = useCallback(() => {
    Stores.File.__state.setFileFindOpen(fileId, false)
    // Restore focus into the viewer so keyboard focus doesn't drop to <body>.
    regionRef.current?.focus()
  }, [fileId])

  // Register this region for the document-level Ctrl-F shortcut while mounted.
  useEffect(() => {
    if (!supported) return
    const el = regionRef.current
    if (!el) return
    return registerRegion({ el, open: openFind })
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
