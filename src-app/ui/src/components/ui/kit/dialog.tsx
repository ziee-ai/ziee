import * as React from 'react'
import {
  Dialog as Root, DialogTrigger, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription,
  HOST_FOCUS_TRAP_SELECTOR,
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
  // If this Dialog lives inside a host focus-trap (a vaul Drawer), portal the
  // popup INTO that trap so its focus scope doesn't steal focus from — and
  // silence onChange on — inputs in the popup. Resolved from an always-mounted
  // inline anchor (which sits in the trap's DOM subtree when there is one, else
  // nowhere → `null` → base-ui's default `<body>` portal). See dialog.tsx's
  // HOST_FOCUS_TRAP_SELECTOR note.
  const anchorRef = React.useRef<HTMLSpanElement>(null)
  const [container, setContainer] = React.useState<HTMLElement | null>(null)
  React.useLayoutEffect(() => {
    setContainer(anchorRef.current?.closest<HTMLElement>(HOST_FOCUS_TRAP_SELECTOR) ?? null)
  }, [open])
  return (
    <Root open={open} onOpenChange={onOpenChange}>
      <span ref={anchorRef} aria-hidden className="hidden" />
      {trigger != null && <DialogTrigger render={trigger} />}
      {/* When no description, tell Radix the omission is intentional (suppresses its dev warning);
          when a description exists, let Radix auto-wire aria-describedby to it. */}
      <DialogContent container={container ?? undefined} className={cn(widths[size], className)} data-testid={testid} {...(description == null ? { 'aria-describedby': undefined } : {})}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description != null && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {children}
        {footer != null && <DialogFooter>{footer}</DialogFooter>}
      </DialogContent>
    </Root>
  )
}
