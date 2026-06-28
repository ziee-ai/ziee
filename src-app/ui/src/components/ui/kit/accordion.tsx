import * as React from 'react'
import { Accordion as Root, AccordionItem, AccordionTrigger, AccordionContent } from '../shadcn/accordion'
import { useSurface } from './surface'
import { cn } from '@/lib/utils'

export interface AccordionItemDef {
  key: string
  label: React.ReactNode
  children?: React.ReactNode
  disabled?: boolean
}

export type AccordionProps =
  | { items: AccordionItemDef[]; type?: 'single'; collapsible?: boolean; defaultValue?: string; value?: string; onValueChange?: (v: string) => void; ghost?: boolean; className?: string; 'data-testid'?: string }
  | { items: AccordionItemDef[]; type: 'multiple'; defaultValue?: string[]; value?: string[]; onValueChange?: (v: string[]) => void; ghost?: boolean; className?: string; 'data-testid'?: string }

// legacy `ghost` removes item borders/background.
const ghostCls = '[&_[data-slot=accordion-item]]:border-0'

function renderItems(items: AccordionItemDef[], surfaceDisabled: boolean) {
  return items.map((it) => (
    <AccordionItem key={it.key} value={it.key} disabled={it.disabled || surfaceDisabled}>
      <AccordionTrigger>{it.label}</AccordionTrigger>
      <AccordionContent>{it.children}</AccordionContent>
    </AccordionItem>
  ))
}

export function Accordion(props: AccordionProps) {
  const { items, className, ghost } = props
  const testid = props['data-testid']
  const cls = cn(ghost && ghostCls, className)
  // react to an ambient disabled surface (e.g. inside a disabled Form/Card).
  const s = useSurface({})
  if (props.type === 'multiple') {
    return (
      <Root type="multiple" value={props.value} defaultValue={props.defaultValue} onValueChange={props.onValueChange} className={cls} data-testid={testid}>
        {renderItems(items, !!s.disabled)}
      </Root>
    )
  }
  // single — `collapsible` is only valid on the single variant.
  return (
    <Root type="single" collapsible={props.collapsible ?? true} value={props.value} defaultValue={props.defaultValue} onValueChange={props.onValueChange} className={cls} data-testid={testid}>
      {renderItems(items, !!s.disabled)}
    </Root>
  )
}
