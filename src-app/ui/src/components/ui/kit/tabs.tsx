import * as React from 'react'
import { Plus, X } from 'lucide-react'
import { Tabs as Root, TabsList, TabsTrigger, TabsContent } from '../shadcn/tabs'
import { useSurface } from './surface'
import { cn } from '@/lib/utils'

export interface TabItem {
  key: string
  label: React.ReactNode
  children?: React.ReactNode
  disabled?: boolean
  /** Show a close (×) affordance on this tab. Defaults to true when the Tabs is `editable`. */
  closable?: boolean
}

export interface TabsProps {
  items: TabItem[]
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  /** Fires when a tab trigger is clicked (legacy `onTabClick`), even if already active. */
  onTabClick?: (key: string) => void
  disabled?: boolean
  size?: 'sm' | 'default'
  className?: string
  /** Editable-card mode: renders an add button + a per-tab close affordance (legacy `type="editable-card"`). */
  editable?: boolean
  /** Hide the add button while keeping per-tab close affordances. */
  hideAdd?: boolean
  /** Unified edit handler (legacy antd `onEdit`): action is 'add' or 'remove'; key is '' for add. */
  onEdit?: (action: 'add' | 'remove', key: string) => void
  /** Fires with the key of the tab whose close affordance was activated. */
  onClose?: (key: string) => void
  /** Accessible name for the add button. Falls back to "Add tab" if omitted. */
  addLabel?: string
  /** Accessible name for a tab's close affordance. Falls back to "Close <label>". */
  closeLabel?: (item: TabItem) => string
}

export function Tabs({
  items, value, defaultValue, onValueChange, onTabClick, disabled, size, className,
  editable, hideAdd, onEdit, onClose, addLabel, closeLabel,
}: TabsProps) {
  // React to an ambient disabled surface (e.g. inside a disabled Form/Card).
  const s = useSurface({ disabled })

  const remove = (item: TabItem) => {
    onClose?.(item.key)
    onEdit?.('remove', item.key)
  }
  const add = () => onEdit?.('add', '')

  return (
    <Root
      value={value}
      defaultValue={value === undefined ? (defaultValue ?? items[0]?.key) : undefined}
      onValueChange={onValueChange}
      className={cn('w-full', className)}
    >
      <TabsList>
        {items.map((t) => {
          const showClose = (t.closable ?? editable) && !s.disabled && !t.disabled
          return (
            <TabsTrigger
              key={t.key}
              value={t.key}
              disabled={t.disabled || s.disabled}
              onClick={() => onTabClick?.(t.key)}
              className={cn(size === 'sm' && 'px-2 py-1 text-xs', showClose && 'gap-1.5')}
            >
              {t.label}
              {showClose && (
                // role=button (not a nested <button>, which is invalid inside the trigger button).
                <span
                  role="button"
                  tabIndex={0}
                  aria-label={closeLabel ? closeLabel(t) : `Close ${typeof t.label === 'string' ? t.label : t.key}`}
                  // stop the activation from also selecting the tab.
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={(e) => { e.stopPropagation(); remove(t) }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); remove(t) }
                  }}
                  className="inline-flex size-4 items-center justify-center rounded-sm opacity-60 hover:bg-accent hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <X className="size-3" aria-hidden />
                </span>
              )}
            </TabsTrigger>
          )
        })}
        {editable && !hideAdd && (
          <button
            type="button"
            aria-label={addLabel ?? 'Add tab'}
            disabled={s.disabled}
            onClick={add}
            className="ml-1 inline-flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
          >
            <Plus className="size-4" aria-hidden />
          </button>
        )}
      </TabsList>
      {items.map((t) => (
        <TabsContent key={t.key} value={t.key}>
          {t.children}
        </TabsContent>
      ))}
    </Root>
  )
}
