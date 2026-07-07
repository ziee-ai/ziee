import { ChevronLeft, ChevronRight, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Button, Input } from '@/components/ui'

/**
 * The find-in-document control strip. Presentational — the match state comes
 * from `useFindInDocument` (owned by FindableRegion), which this bar drives.
 *
 * Keyboard: Enter → next, Shift+Enter → prev, Escape → close.
 */
export function FindBar({
  query,
  onQueryChange,
  count,
  activeIndex,
  onNext,
  onPrev,
  onClose,
}: {
  query: string
  onQueryChange: (q: string) => void
  count: number
  activeIndex: number
  onNext: () => void
  onPrev: () => void
  onClose: () => void
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  // Focus the field when the bar mounts (opened) so the user can type immediately.
  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])

  return (
    <div
      className="flex items-center gap-1 px-2 py-1.5 border-border border-b bg-muted/60 flex-shrink-0"
      data-testid="file-find-bar"
    >
      <Input
        ref={inputRef}
        size="sm"
        value={query}
        placeholder="Find in document"
        aria-label="Find in document"
        data-testid="file-find-input"
        // flex-1 (mirrors the PDF viewer's find bar): the input grows to fill
        // the strip so the count + nav buttons pin to the right edge, instead of
        // packing everything left. No search-icon prefix — the PDF bar's input
        // is bare, so these match. ms-1: a little breathing room from the strip's
        // left edge.
        className="flex-1 ms-1"
        onChange={e => onQueryChange(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') {
            e.preventDefault()
            if (e.shiftKey) onPrev()
            else onNext()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            onClose()
          }
        }}
      />
      {/* Count reads "N of M" (hidden while the query is empty), matching the
          PDF viewer's find bar — no reserved-width box, no "No results" text
          (0 of M conveys the same). */}
      <span
        className="ms-2 text-xs text-muted-foreground tabular-nums whitespace-nowrap"
        data-testid="file-find-count"
        aria-live="polite"
      >
        {query === '' ? '' : `${count === 0 ? 0 : activeIndex + 1} of ${count}`}
      </span>
      <Button
        variant="ghost"
        size="icon"
        tooltip="Previous match"
        aria-label="Previous match"
        icon={<ChevronLeft />}
        disabled={count === 0}
        onClick={onPrev}
        data-testid="file-find-prev-btn"
      />
      <Button
        variant="ghost"
        size="icon"
        tooltip="Next match"
        aria-label="Next match"
        icon={<ChevronRight />}
        disabled={count === 0}
        onClick={onNext}
        data-testid="file-find-next-btn"
      />
      <Button
        variant="ghost"
        size="icon"
        tooltip="Close find"
        aria-label="Close find"
        icon={<X />}
        onClick={onClose}
        data-testid="file-find-close-btn"
      />
    </div>
  )
}
