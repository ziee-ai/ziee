import * as React from 'react'
import {
  DropdownMenu as Root, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuLabel,
} from '../shadcn/dropdown-menu'
import { cn } from '@/lib/utils'

export type DropdownItem =
  | { type: 'divider' }
  | { type: 'label'; label: React.ReactNode }
  | {
      key: string
      label: React.ReactNode
      icon?: React.ReactNode
      onClick?: () => void
      danger?: boolean
      disabled?: boolean
    }

export interface DropdownProps {
  items: DropdownItem[]
  children: React.ReactElement
  side?: 'top' | 'right' | 'bottom' | 'left'
  align?: 'start' | 'center' | 'end'
  /** Disables the trigger (legacy `disabled`). */
  disabled?: boolean
  /** Global selection handler receiving the activated item's `key` (legacy `menu.onClick`).
   *  Fires in addition to a per-item `onClick`. */
  onSelect?: (key: string) => void
}

export function Dropdown({ items, children, side, align = 'end', disabled, onSelect }: DropdownProps) {
  return (
    <Root>
      <DropdownMenuTrigger asChild disabled={disabled}>{children}</DropdownMenuTrigger>
      <DropdownMenuContent side={side} align={align}>
        {items.map((it, i) =>
          'type' in it && it.type === 'divider' ? (
            <DropdownMenuSeparator key={`d${i}`} />
          ) : 'type' in it && it.type === 'label' ? (
            <DropdownMenuLabel key={`l${i}`}>{it.label}</DropdownMenuLabel>
          ) : (
            <DropdownMenuItem
              key={(it as { key: string }).key}
              disabled={(it as { disabled?: boolean }).disabled}
              onClick={() => {
                ;(it as { onClick?: () => void }).onClick?.()
                onSelect?.((it as { key: string }).key)
              }}
              className={cn((it as { danger?: boolean }).danger && 'text-destructive focus:text-destructive')}
            >
              {(it as { icon?: React.ReactNode }).icon != null && (
                <span aria-hidden>{(it as { icon?: React.ReactNode }).icon}</span>
              )}
              {(it as { label: React.ReactNode }).label}
            </DropdownMenuItem>
          ),
        )}
      </DropdownMenuContent>
    </Root>
  )
}
