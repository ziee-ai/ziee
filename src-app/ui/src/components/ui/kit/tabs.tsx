import * as React from 'react'
import { Tabs as Root, TabsList, TabsTrigger, TabsContent } from '../shadcn/tabs'
import { useSurface } from './surface'
import { cn } from '@/lib/utils'

export interface TabItem {
  key: string
  label: React.ReactNode
  children?: React.ReactNode
  disabled?: boolean
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
}

export function Tabs({ items, value, defaultValue, onValueChange, onTabClick, disabled, size, className }: TabsProps) {
  // React to an ambient disabled surface (e.g. inside a disabled Form/Card).
  const s = useSurface({ disabled })
  return (
    <Root
      value={value}
      defaultValue={value === undefined ? (defaultValue ?? items[0]?.key) : undefined}
      onValueChange={onValueChange}
      className={cn('w-full', className)}
    >
      <TabsList>
        {items.map((t) => (
          <TabsTrigger
            key={t.key}
            value={t.key}
            disabled={t.disabled || s.disabled}
            onClick={() => onTabClick?.(t.key)}
            className={cn(size === 'sm' && 'px-2 py-1 text-xs')}
          >
            {t.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {items.map((t) => (
        <TabsContent key={t.key} value={t.key}>
          {t.children}
        </TabsContent>
      ))}
    </Root>
  )
}
