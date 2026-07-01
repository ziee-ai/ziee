import * as React from 'react'
import {
  DropdownMenu as Root, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuLabel, DropdownMenuGroup,
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
  /** Controlled open state. Omit for the default uncontrolled behavior. */
  open?: boolean
  /** Fires when the menu requests an open-state change (pairs with `open`). */
  onOpenChange?: (open: boolean) => void
  /** Initial open state when uncontrolled (legacy `defaultOpen`). */
  defaultOpen?: boolean
  /** Test selector — forwarded onto the menu content <root>. Items derive `${testid}-item-${key}`. */
  'data-testid': string
}

export function Dropdown({ items, children, side, align = 'end', disabled, onSelect, open, onOpenChange, defaultOpen, 'data-testid': testid }: DropdownProps) {
  // Base UI's trigger defaults to `nativeButton: true` and warns if the rendered
  // element isn't a real <button>. Our trigger is a caller-supplied element that
  // may be a native <button>, a component (e.g. kit <Button>, which renders one),
  // or a bare <div role="button"> (legacy Radix pattern). Only a literal
  // intrinsic that isn't 'button' needs nativeButton=false — then Base UI supplies
  // the button ARIA/keyboard semantics on the non-button element.
  const childType = (children as React.ReactElement)?.type
  const nativeButton = typeof childType === 'string' ? childType === 'button' : true
  return (
    <Root open={open} onOpenChange={onOpenChange} defaultOpen={defaultOpen}>
      <DropdownMenuTrigger render={children} disabled={disabled} nativeButton={nativeButton} />
      <DropdownMenuContent side={side} align={align} data-testid={testid}>
        {items.map((it, i) =>
          'type' in it && it.type === 'divider' ? (
            <DropdownMenuSeparator key={`d${i}`} />
          ) : 'type' in it && it.type === 'label' ? (
            // Base UI's GroupLabel requires a Group ancestor (throws
            // "MenuGroupContext is missing" otherwise), so wrap the section
            // label in its own group.
            <DropdownMenuGroup key={`l${i}`}>
              <DropdownMenuLabel>{it.label}</DropdownMenuLabel>
            </DropdownMenuGroup>
          ) : (
            <DropdownMenuItem
              key={(it as { key: string }).key}
              data-testid={testid ? `${testid}-item-${(it as { key: string }).key}` : undefined}
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
