import { useCallback, useRef, useState } from 'react'
import { Stores } from '@/core/stores'
import { FindBar } from './FindBar'
import { useFindInDocument } from './useFindInDocument'
import { isHighlightSupported } from './highlightSupported'

/**
 * Wraps a viewer body in find-in-document. Renders the FindBar (when open) above
 * the content, paints matches via the CSS Custom Highlight API, and captures
 * Ctrl/Cmd-F WITHIN this region only (native find stays intact elsewhere).
 *
 * Open-state is coordinated with the header FindButton through
 * `File.store.fileFindOpen` (header + body are sibling components — the store is
 * the shared surface, matching the fileViewModes idiom).
 *
 * When the Highlight API is unavailable the region is a passthrough: no bar, no
 * key capture, so the browser's native find takes over.
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

  const active = supported && open
  const { count, activeIndex, next, prev } = useFindInDocument(
    contentRef,
    query,
    active,
  )

  const close = useCallback(() => {
    Stores.File.__state.setFileFindOpen(fileId, false)
  }, [fileId])

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!supported) return
      if ((e.ctrlKey || e.metaKey) && (e.key === 'f' || e.key === 'F')) {
        e.preventDefault()
        Stores.File.__state.setFileFindOpen(fileId, true)
      }
    },
    [supported, fileId],
  )

  return (
    // The Highlight registrations are process-global; the <style> scopes their
    // paint. tabIndex lets the region receive the keydown even before the input
    // is focused (e.g. Ctrl-F while the pointer is over the body).
    <div
      className={className ?? 'flex flex-col h-full w-full min-h-0'}
      onKeyDown={onKeyDown}
      tabIndex={-1}
      data-testid="file-findable-region"
    >
      {supported && (
        <style>{`
          ::highlight(file-find) { background-color: color-mix(in srgb, var(--warning) 40%, transparent); }
          ::highlight(file-find-active) { background-color: color-mix(in srgb, var(--warning) 85%, transparent); }
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
