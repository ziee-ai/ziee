import * as React from 'react'
import { useSurface } from './surface'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export type MenuItem =
  | { type: 'divider' }
  | { type: 'group'; label: React.ReactNode; children: MenuItem[] }
  | { type: 'label'; label: React.ReactNode }
  | {
      key: string
      label: React.ReactNode
      icon?: React.ReactNode
      disabled?: boolean
      /** Explicit accessible name — REQUIRED for a non-string label in `collapsed` mode
       *  (the label is hidden then). Preferred over the label/key fallbacks. */
      title?: string
    }

// legacy Menu (navigation subset): vertical/horizontal item list with single selection.
// Rendered as a <nav> + roving list; items are real buttons. `aria-label` required (i18n).
export type MenuProps = {
  items: MenuItem[]
  /** Selected item key (single-selection). */
  selectedKey?: string
  /** Selected item keys (alias of `selectedKey`; any match marks the item current). */
  selectedKeys?: string[]
  onSelect?: (key: string) => void
  mode?: 'vertical' | 'horizontal'
  /** Icon-only rail: hides labels (the label becomes each item's accessible name). */
  collapsed?: boolean
  className?: string
  'aria-label': string
  /** Test selector — forwarded onto <root>. Items derive `${testid}-item-${key}`. */
  'data-testid': string
} & KitStyleProps

// Extract a plain-text accessible name from a label node (used when `collapsed` hides it).
function labelText(label: React.ReactNode): string | undefined {
  return typeof label === 'string' ? label : undefined
}

function Items({ items, selectedSet, onSelect, locked, collapsed, itemTestid, groupTestid }: {
  items: MenuItem[]
  selectedSet: Set<string>
  onSelect?: (k: string) => void
  locked: boolean
  collapsed: boolean
  itemTestid?: (key: string) => string | undefined
  groupTestid?: (index: number) => string | undefined
}) {
  return (
    <>
      {items.map((it, i) => {
        // The <li> must keep its implicit listitem role (a role="separator" on the
        // li makes the parent <ul> contain a non-listitem child → axe `list`
        // violation). Put the separator on an inner element instead.
        if ('type' in it && it.type === 'divider')
          return (
            <li key={`d${i}`} className="my-1">
              <div role="separator" className="h-px bg-border" />
            </li>
          )
        if ('type' in it && it.type === 'group') {
          return (
            <li key={`g${i}`} data-testid={groupTestid?.(i)}>
              {/* group caption is decorative chrome — hidden in the collapsed rail. */}
              {!collapsed && <div className="px-3 py-1 text-xs font-medium text-muted-foreground">{it.label}</div>}
              <ul className="contents">
                <Items items={it.children} selectedSet={selectedSet} onSelect={onSelect} locked={locked} collapsed={collapsed} itemTestid={itemTestid} groupTestid={groupTestid} />
              </ul>
            </li>
          )
        }
        // Non-interactive caption row (legacy antd `{ type: 'group' }` standalone label).
        if ('type' in it && it.type === 'label') {
          return collapsed ? null : (
            <li key={`l${i}`} className="px-3 py-1 text-xs font-medium text-muted-foreground">
              {it.label}
            </li>
          )
        }
        const item = it
        const selected = selectedSet.has(item.key)
        // Never nameless in collapsed mode: explicit title → string label → the key.
        const name = item.title ?? labelText(item.label) ?? item.key
        return (
          <li key={item.key}>
            <button
              type="button"
              disabled={item.disabled || locked}
              data-testid={itemTestid?.(item.key)}
              aria-current={selected ? 'page' : undefined}
              // collapsed hides the text → the label becomes the button's accessible name + tooltip.
              aria-label={collapsed ? name : undefined}
              title={collapsed ? name : undefined}
              onClick={() => onSelect?.(item.key)}
              className={cn(
                'flex w-full min-w-0 items-center gap-2 rounded-md text-sm',
                collapsed ? 'justify-center px-2 py-1.5' : 'px-3 py-1.5',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50',
                selected ? 'bg-accent font-medium' : 'hover:bg-accent/60',
              )}
            >
              {item.icon != null && <span aria-hidden className="shrink-0 [&_svg]:size-4">{item.icon}</span>}
              {/* truncate long labels instead of overflowing the rail. */}
              {!collapsed && <span className="min-w-0 flex-1 truncate text-left">{item.label}</span>}
            </button>
          </li>
        )
      })}
    </>
  )
}

export function Menu({ items, selectedKey, selectedKeys, onSelect, mode = 'vertical', collapsed = false, className, style, 'aria-label': ariaLabel, 'data-testid': testid }: MenuProps) {
  const s = useSurface({})
  const itemTestid = React.useCallback(
    (k: string) => (testid ? `${testid}-item-${k}` : undefined),
    [testid],
  )
  const groupTestid = React.useCallback(
    (index: number) => (testid ? `${testid}-group-${index}` : undefined),
    [testid],
  )
  // Merge the single + multi selection inputs into one O(1) lookup set.
  const selectedSet = React.useMemo(() => {
    const set = new Set(selectedKeys ?? [])
    if (selectedKey != null) set.add(selectedKey)
    return set
  }, [selectedKey, selectedKeys])
  return (
    <nav aria-label={ariaLabel} style={style} data-testid={testid}>
      <ul className={cn(mode === 'horizontal' ? 'flex items-center gap-1' : 'flex flex-col gap-0.5', className)}>
        <Items items={items} selectedSet={selectedSet} onSelect={onSelect} locked={!!s.disabled} collapsed={collapsed} itemTestid={itemTestid} groupTestid={groupTestid} />
      </ul>
    </nav>
  )
}
