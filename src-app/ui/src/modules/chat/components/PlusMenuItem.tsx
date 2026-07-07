import { forwardRef, type ComponentPropsWithoutRef, type ReactNode } from 'react'
import { cn } from '@/lib/utils'

type PlusMenuItemProps = {
  icon: ReactNode
  label: ReactNode
  /** Trailing adornment (e.g. a `<ChevronRight>` submenu indicator). */
  trailing?: ReactNode
  'data-testid': string
} & Omit<ComponentPropsWithoutRef<'div'>, 'children'>

/**
 * Shared row for an item inside the chat composer "+" dropdown.
 *
 * Every "+" item MUST share the same metrics (taxonomy A9), so the icon size and
 * layout are OWNED here, not set per-item:
 * - leading icon forced to `size-4` and `shrink-0` (never squished, never wraps
 *   off the label),
 * - `text-sm` label on ONE line — `truncate` + the row's `whitespace-nowrap` keep
 *   icon+label together even in a narrow chat column (they ellipsize, never wrap
 *   onto a second row),
 * - consistent `px-3 py-1.5` padding + `hover:bg-muted`,
 * - optional `trailing` slot (e.g. a submenu chevron), pinned right via `ml-auto`.
 *
 * forwardRef + prop spread so it can also serve as a Dropdown/Popover TRIGGER
 * (the Export item wraps it in the export-format Dropdown). Single-row items
 * (Skills / MCP / Export) render this directly; the list-style Assistant item
 * reuses the same wrapper classes on its trigger.
 */
export const PlusMenuItem = forwardRef<HTMLDivElement, PlusMenuItemProps>(
  function PlusMenuItem({ icon, label, trailing, onClick, onKeyDown, className, ...rest }, ref) {
    return (
      <div
        ref={ref}
        role="button"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={e => {
          if (onClick && (e.key === 'Enter' || e.key === ' ')) {
            e.preventDefault()
            onClick(e as unknown as React.MouseEvent<HTMLDivElement>)
          }
          onKeyDown?.(e)
        }}
        className={cn(
          'flex items-center gap-2 px-3 py-1.5 rounded-md cursor-pointer text-foreground hover:bg-muted focus-visible:outline focus-visible:outline-2 whitespace-nowrap',
          className,
        )}
        {...rest}
      >
        <span className="shrink-0 inline-flex items-center [&_svg]:size-4">{icon}</span>
        <span className="min-w-0 flex-1 truncate text-sm">{label}</span>
        {trailing != null && (
          <span className="ml-auto shrink-0 inline-flex items-center pl-2">
            {trailing}
          </span>
        )}
      </div>
    )
  },
)
