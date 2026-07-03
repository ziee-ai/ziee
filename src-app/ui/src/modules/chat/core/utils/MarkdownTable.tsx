import {
  memo,
  useCallback,
  useEffect,
  useState,
  type JSX,
} from 'react'
import { createPortal } from 'react-dom'
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react'
import { Maximize2 } from 'lucide-react'
import { TableCopyDropdown, TableDownloadDropdown } from 'streamdown'
import { Button } from '@/components/ui'
import { cn } from '@/lib/utils'

/**
 * Replacement for Streamdown's built-in table wrapper (`components.table`).
 *
 * Overriding `table` swaps out Streamdown's whole wrapper — which brought a
 * NATIVE `overflow-x-auto` scroller AND a fullscreen control that portals a
 * `fixed inset-0` overlay into document.body. We rebuild it so that:
 *   1. horizontal scrolling uses the app's OverlayScrollbars (auto-hide),
 *      matching every other scroller in the app, and
 *   2. the "fullscreen" affordance instead opens a real blank popup WINDOW and
 *      React-portals the live table into it (a separate browser tab/window,
 *      not an in-page overlay).
 *
 * Copy / download are preserved by reusing Streamdown's own exported controls
 * (`TableCopyDropdown` / `TableDownloadDropdown`), which locate the table via
 * the `data-streamdown="table-wrapper"` ancestor we still set here.
 */

const CONTROL_BTN =
  'inline-flex items-center justify-center rounded-md size-7 text-muted-foreground ' +
  'hover:bg-muted hover:text-foreground transition-colors cursor-pointer'

/** Clone the opener's stylesheets into a popup document so the portaled table
 *  renders with the same Tailwind/shadcn styling. Covers both dev (`<style>`
 *  injected by Vite) and prod (`<link rel=stylesheet>`). */
function copyStylesInto(target: Document) {
  const nodes = document.querySelectorAll(
    'style, link[rel="stylesheet"]',
  )
  for (const node of Array.from(nodes)) {
    target.head.appendChild(node.cloneNode(true))
  }
}

type TableProps = JSX.IntrinsicElements['table'] & { node?: unknown }

export const MarkdownTable = memo(function MarkdownTable({
  children,
  className,
  node: _node,
  ...rest
}: TableProps) {
  const [popup, setPopup] = useState<{
    win: Window
    container: HTMLElement
  } | null>(null)

  const openPopout = useCallback(() => {
    const win = window.open(
      '',
      '_blank',
      'width=1024,height=720,scrollbars=yes',
    )
    // Popup blocked (should not happen on a direct click gesture) — no-op.
    if (!win) return
    win.document.title = 'Table'
    copyStylesInto(win.document)
    // Mirror the theme: the light/dark class + data-theme live on <html>.
    win.document.documentElement.className =
      document.documentElement.className
    const theme = document.documentElement.getAttribute('data-theme')
    if (theme) win.document.documentElement.setAttribute('data-theme', theme)
    win.document.body.className = 'bg-background text-foreground'
    const container = win.document.createElement('div')
    container.className = 'p-6'
    win.document.body.appendChild(container)
    win.addEventListener('beforeunload', () => setPopup(null))
    setPopup({ win, container })
  }, [])

  // Close the popup if this table unmounts (e.g. navigating away mid-view).
  useEffect(() => {
    return () => {
      popup?.win.close()
    }
  }, [popup])

  // A single element description rendered in BOTH the inline scroller and the
  // popup portal — React mounts it independently in each target.
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
          tooltip="Open table in a new window"
          icon={<Maximize2 className="size-3.5" />}
          onClick={openPopout}
          data-testid="markdown-table-popout-btn"
        />
      </div>

      <OverlayScrollbarsComponent
        options={{
          scrollbars: { autoHide: 'leave' },
          overflow: { y: 'hidden' },
        }}
        className="overflow-x-auto rounded-lg border border-border"
        defer
      >
        {table}
      </OverlayScrollbarsComponent>

      {popup &&
        createPortal(
          <div className="w-full overflow-auto">{table}</div>,
          popup.container,
        )}
    </div>
  )
})
