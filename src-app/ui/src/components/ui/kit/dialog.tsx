import * as React from 'react'
import {
  Dialog as Root, DialogTrigger, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription,
} from '../shadcn/dialog'
import { cn } from '@/lib/utils'

const widths = { sm: 'sm:max-w-sm', default: 'sm:max-w-lg', lg: 'sm:max-w-2xl', xl: 'sm:max-w-4xl' } as const

export interface DialogProps {
  open?: boolean
  onOpenChange?: (open: boolean) => void
  /** Accessible name — required (Radix Dialog must be labelled). */
  title: React.ReactNode
  description?: React.ReactNode
  footer?: React.ReactNode
  size?: keyof typeof widths
  trigger?: React.ReactElement
  className?: string
  children?: React.ReactNode
  /** Test selector — forwarded onto the dialog content <root> (i18n-safe). */
  'data-testid': string
}

export function Dialog({ open, onOpenChange, title, description, footer, size = 'default', trigger, className, children, 'data-testid': testid }: DialogProps) {
  return (
    <Root open={open} onOpenChange={onOpenChange}>
      {trigger != null && <DialogTrigger render={trigger} />}
      {/* When no description, tell Radix the omission is intentional (suppresses its dev warning);
          when a description exists, let Radix auto-wire aria-describedby to it. */}
      {/* The DialogContent ITSELF is the scroll container (flex-col, capped at
          the viewport). It's sized to its own content, so when content fits there
          is genuinely no overflow — no phantom scrollbar. Only a dialog taller
          than the viewport scrolls, with the header + footer sticky-pinned.
          (An inner scroll box was ~2px short of its content due to flex/OS
          rounding, so it always looked scrollable.) */}
      <DialogContent
        className={cn('max-h-[calc(100dvh-2rem)] !flex flex-col overflow-y-auto overflow-x-hidden', widths[size], className)}
        data-testid={testid}
        {...(description == null ? { 'aria-describedby': undefined } : {})}
      >
        <DialogHeader className="sticky top-0 z-10 shrink-0 bg-popover">
          <DialogTitle>{title}</DialogTitle>
          {description != null && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {children}
        {footer != null && (
          <DialogFooter className="sticky bottom-0 z-10 shrink-0 bg-popover">{footer}</DialogFooter>
        )}
      </DialogContent>
    </Root>
  )
}
