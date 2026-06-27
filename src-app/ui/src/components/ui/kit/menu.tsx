import * as React from 'react'
import { useSurface } from './surface'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export type MenuItem =
  | { type: 'divider' }
  | { type: 'group'; label: React.ReactNode; children: MenuItem[] }
  | { key: string; label: React.ReactNode; icon?: React.ReactNode; disabled?: boolean }

// legacy Menu (navigation subset): vertical/horizontal item list with single selection.
// Rendered as a <nav> + roving list; items are real buttons. `aria-label` required (i18n).
export type MenuProps = {
  items: MenuItem[]
  selectedKey?: string
  onSelect?: (key: string) => void
  mode?: 'vertical' | 'horizontal'
  className?: string
  'aria-label': string
} & KitStyleProps

function Items({ items, selectedKey, onSelect, locked }: {
  items: MenuItem[]; selectedKey?: string; onSelect?: (k: string) => void; locked: boolean
}) {
  return (
    <>
      {items.map((it, i) => {
        if ('type' in it && it.type === 'divider') return <li key={`d${i}`} role="separator" className="my-1 h-px bg-border" />
        if ('type' in it && it.type === 'group') {
          return (
            <li key={`g${i}`}>
              <div className="px-3 py-1 text-xs font-medium text-muted-foreground">{it.label}</div>
              <ul className="contents">
                <Items items={it.children} selectedKey={selectedKey} onSelect={onSelect} locked={locked} />
              </ul>
            </li>
          )
        }
        const item = it
        const selected = selectedKey === item.key
        return (
          <li key={item.key}>
            <button
              type="button"
              disabled={item.disabled || locked}
              aria-current={selected ? 'page' : undefined}
              onClick={() => onSelect?.(item.key)}
              className={cn(
                'flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50',
                selected ? 'bg-accent font-medium' : 'hover:bg-accent/60',
              )}
            >
              {item.icon != null && <span aria-hidden className="[&_svg]:size-4">{item.icon}</span>}
              {item.label}
            </button>
          </li>
        )
      })}
    </>
  )
}

export function Menu({ items, selectedKey, onSelect, mode = 'vertical', className, style, 'aria-label': ariaLabel }: MenuProps) {
  const s = useSurface({})
  return (
    <nav aria-label={ariaLabel} style={style}>
      <ul className={cn(mode === 'horizontal' ? 'flex items-center gap-1' : 'flex flex-col gap-0.5', className)}>
        <Items items={items} selectedKey={selectedKey} onSelect={onSelect} locked={!!s.disabled} />
      </ul>
    </nav>
  )
}
