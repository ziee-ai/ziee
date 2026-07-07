import { lazy, memo, Suspense, useEffect, useState, type JSX } from 'react'
import { createPortal } from 'react-dom'
import { Maximize2, X } from 'lucide-react'
import { Button, ScrollArea } from '@/components/ui'
import { cn } from '@/lib/utils'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

// Load Streamdown's copy/download controls dynamically so this file carries NO
// static `import … from 'streamdown'`. That is what lets the whole `streamdown`
// package (Shiki + micromark/mdast/rehype + parse5) split into its own lazy
// chunk instead of being pulled into the initial entry bundle (see
// LazyStreamdown.tsx). Safe because MarkdownTable is a `components.table`
// renderer that ONLY ever mounts inside an already-loaded Streamdown tree, so
// the chunk is present by the time these render. The loaders go through
// `lazyWithPreload` so the desktop webview preloads them (embedded chunk) while
// web/tunnel builds keep the deferred download.
const loadCopyDropdown = lazyWithPreload(() =>
  import('streamdown').then(m => ({ default: m.TableCopyDropdown })),
)
const loadDownloadDropdown = lazyWithPreload(() =>
  import('streamdown').then(m => ({ default: m.TableDownloadDropdown })),
)
const TableCopyDropdown = lazy(loadCopyDropdown)
const TableDownloadDropdown = lazy(loadDownloadDropdown)

/**
 * Replacement for Streamdown's built-in table wrapper (`components.table`).
 *
 * Overriding `table` swaps out Streamdown's whole wrapper — which brought a
 * NATIVE `overflow-x-auto` scroller AND a fullscreen control that portals a
 * `fixed inset-0 z-50` overlay into document.body — far below the file-preview
 * drawer, which opens elevated at z-1050. We rebuild it so:
 *   1. horizontal scrolling uses the app's OverlayScrollbars (auto-hide), and
 *   2. "fullscreen" portals an in-page overlay at z-[1200] — above the file
 *      drawer (z-1050) + chat right panel (z-1000), below tooltips (z-[2000]) —
 *      instead of Streamdown's z-50 one.
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
    // Card layout mirroring Streamdown's code block: a bordered container with
    // an ALWAYS-VISIBLE header bar (controls live OUTSIDE the table, like the
    // code-block toolbar) above the scrollable table body.
    <div
      data-streamdown="table-wrapper"
      className="my-4 flex w-full flex-col gap-2 rounded-xl border border-border bg-sidebar p-2"
    >
      <div className="flex h-8 items-center justify-between text-muted-foreground text-xs">
        <span className="ml-1 font-mono lowercase">table</span>
        <div className="flex items-center gap-0.5">
          <Suspense fallback={null}>
            <TableCopyDropdown className={CONTROL_BTN} />
            <TableDownloadDropdown className={CONTROL_BTN} />
          </Suspense>
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
      </div>

      <ScrollArea
        axis="both"
        autoHide="leave"
        // Cap tall tables so a huge one doesn't dominate the message; scroll
        // inside beyond that. Keep in sync with index.css's code-block cap.
        className="max-h-[min(60vh,36rem)] rounded-md border border-border bg-background"
      >
        {table}
      </ScrollArea>

      {fullscreen &&
        createPortal(
          <div
            // pointer-events-auto: a modal Radix Dialog (the file drawer) sets
            // body { pointer-events: none }, which this body-portal sibling would
            // otherwise inherit — making the overlay + Exit button unclickable.
            className="pointer-events-auto fixed inset-0 z-[1200] flex flex-col bg-background"
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
