import * as React from 'react'
import {
  Dialog as Root, DialogTrigger, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription,
} from '../shadcn/dialog'
import { DivScrollY } from '@/components/common/DivScrollY'
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
      {/* Cap height to the viewport and give the body its own scroll region so a
          tall dialog on a short screen scrolls instead of overflowing off-screen.
          grid-rows keeps the header + footer pinned while the middle row scrolls. */}
      <DialogContent
        className={cn('max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto]', widths[size], className)}
        data-testid={testid}
        {...(description == null ? { 'aria-describedby': undefined } : {})}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description != null && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {/* -mx-4 px-4 breaks the scroll region out to the DialogContent's full
            width so the overlay scrollbar sits flush at the body's right/bottom
            edge (like the settings page), while inner content keeps the p-4
            gutter. overflow.x hidden = never scroll horizontally.
            -my-1.5 py-1.5 does the same vertically: the scroll clip would
            otherwise cut off the focus ring of an input flush at the body's
            top/bottom edge. The negative margin cancels the padding, so no real
            height is added (no new scroll); it just moves the clip edge out past
            the ring. It stays within the 16px gap to the header/footer. */}
        <DivScrollY
          className="min-h-0 -mx-4 px-4 -my-1.5 py-1.5"
          options={{ overflow: { x: 'hidden' } }}
        >
          {children}
        </DivScrollY>
        {footer != null && <DialogFooter>{footer}</DialogFooter>}
      </DialogContent>
    </Root>
  )
}
