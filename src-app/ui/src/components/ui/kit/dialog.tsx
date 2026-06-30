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
      {trigger != null && <DialogTrigger asChild>{trigger}</DialogTrigger>}
      {/* When no description, tell Radix the omission is intentional (suppresses its dev warning);
          when a description exists, let Radix auto-wire aria-describedby to it. */}
      {/* House dialog layout: drop the base's uniform p-6/gap-4 (`!p-0 !gap-0`)
          and give each section its own padding, so the footer can be a distinct
          full-width muted band with a top border. `overflow-hidden` clips that
          band to the rounded corners. */}
      <DialogContent
        className={cn(widths[size], '!gap-0 !p-0 overflow-hidden', className)}
        data-testid={testid}
        {...(description == null ? { 'aria-describedby': undefined } : {})}
      >
        <DialogHeader className="px-5 pt-5 pb-4">
          <DialogTitle>{title}</DialogTitle>
          {description != null && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {children != null && <div className="px-5 pb-5">{children}</div>}
        {footer != null && (
          <DialogFooter className="border-t bg-muted/40 px-5 py-3">
            {footer}
          </DialogFooter>
        )}
      </DialogContent>
    </Root>
  )
}
