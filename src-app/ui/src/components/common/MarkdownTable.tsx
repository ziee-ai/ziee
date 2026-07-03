import { memo, useEffect, useState, type JSX } from 'react'
import { createPortal } from 'react-dom'
import { Maximize2, X } from 'lucide-react'
import { TableCopyDropdown, TableDownloadDropdown } from 'streamdown'
import { Button, ScrollArea } from '@/components/ui'
import { cn } from '@/lib/utils'

/**
 * Replacement for Streamdown's built-in table wrapper (`components.table`).
 *
 * Overriding `table` swaps out Streamdown's whole wrapper — which brought a
 * NATIVE `overflow-x-auto` scroller AND a fullscreen control that portals a
 * `fixed inset-0 z-50` overlay into document.body (too low to clear an open
 * drawer, which is also z-50 but portal-stacked after it). We rebuild it so:
 *   1. horizontal scrolling uses the app's OverlayScrollbars (auto-hide), and
 *   2. "fullscreen" portals an in-page overlay at z-[1000] — above the file
 *      drawer, below tooltips (z-[2000]) — instead of Streamdown's z-50 one.
 *
 * Copy / download are preserved by reusing Streamdown's own exported controls
 * (`TableCopyDropdown` / `TableDownloadDropdown`), which locate the table via
 * the `data-streamdown="table-wrapper"` ancestor we still set here.
 */

const CONTROL_BTN =
  'inline-flex items-center justify-center rounded-md size-7 text-muted-foreground ' +
  'hover:bg-muted hover:text-foreground transition-colors cursor-pointer'

type TableProps = JSX.IntrinsicElements['table'] & { node?: unknown }

export const MarkdownTable = memo(function MarkdownTable({
  children,
  className,
  node: _node,
  ...rest
}: TableProps) {
  const [fullscreen, setFullscreen] = useState(false)

  // Escape exits fullscreen (mirrors Streamdown's own fullscreen behavior).
  useEffect(() => {
    if (!fullscreen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setFullscreen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [fullscreen])

  // One element description rendered in BOTH the inline scroller and the
  // fullscreen overlay — React mounts it independently in each target.
  const table = (
    <table
      data-streamdown="table"
      className={cn('w-full border-collapse text-sm', className)}
      {...rest}
    >
      {children}
    </table>
  )

  return (
    <div
      data-streamdown="table-wrapper"
      className="group/table relative my-4 w-full"
    >
      <div
        className={
          'absolute right-1 top-1 z-10 flex gap-0.5 rounded-md border border-border ' +
          'bg-background/80 p-0.5 backdrop-blur opacity-0 transition-opacity ' +
          'group-hover/table:opacity-100 focus-within:opacity-100 hover-none:opacity-100'
        }
      >
        <TableCopyDropdown className={CONTROL_BTN} />
        <TableDownloadDropdown className={CONTROL_BTN} />
        <Button
          size="icon"
          variant="ghost"
          className="size-7"
          tooltip="View fullscreen"
          icon={<Maximize2 className="size-3.5" />}
          onClick={() => setFullscreen(true)}
          data-testid="markdown-table-fullscreen-btn"
        />
      </div>

      <ScrollArea
        axis="x"
        autoHide="leave"
        className="rounded-lg border border-border"
      >
        {table}
      </ScrollArea>

      {fullscreen &&
        createPortal(
          <div
            // pointer-events-auto: a modal Radix Dialog (the file drawer) sets
            // body { pointer-events: none }, which this body-portal sibling would
            // otherwise inherit — making the overlay + Exit button unclickable.
            className="pointer-events-auto fixed inset-0 z-[1000] flex flex-col bg-background"
            role="dialog"
            aria-modal="true"
            data-testid="markdown-table-fullscreen"
          >
            <div className="flex items-center justify-end border-b border-border p-2">
              <Button
                size="icon"
                variant="ghost"
                tooltip="Exit fullscreen"
                icon={<X />}
                onClick={() => setFullscreen(false)}
                data-testid="markdown-table-fullscreen-exit"
              />
            </div>
            <ScrollArea axis="both" className="flex-1 min-h-0">
              <div className="p-4">{table}</div>
            </ScrollArea>
          </div>,
          document.body,
        )}
    </div>
  )
})
