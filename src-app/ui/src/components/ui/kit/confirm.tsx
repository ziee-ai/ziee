import * as React from 'react'
import {
  AlertDialog as Root, AlertDialogTrigger, AlertDialogContent, AlertDialogHeader, AlertDialogFooter,
  AlertDialogTitle, AlertDialogDescription, AlertDialogCancel,
} from '../shadcn/alert-dialog'
import { Button } from './button'

export interface ConfirmProps {
  title: React.ReactNode
  description?: React.ReactNode
  /** Confirm button text — required (no default, so it's always translatable). */
  okText: string
  /** Cancel button text — required (no default, so it's always translatable). */
  cancelText: string
  danger?: boolean
  onConfirm: () => void | Promise<void>
  /** Called when the user cancels/dismisses (legacy `onCancel`). */
  onCancel?: () => void
  /** Extra props forwarded to the confirm button (legacy `okButtonProps`), e.g. { danger: true }. */
  okButtonProps?: { danger?: boolean; disabled?: boolean }
  children: React.ReactElement
}

// Built on AlertDialog (modal + focus-trapped + focus-restoring), not a Popover — an
// "are you sure?" prompt must trap focus and inert the background.
export function Confirm({ title, description, okText, cancelText, danger, onConfirm, onCancel, okButtonProps, children }: ConfirmProps) {
  const [open, setOpen] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const run = async () => {
    setBusy(true)
    try {
      await onConfirm()
      setOpen(false)
    } catch {
      // keep the dialog open so the user can retry; caller surfaces the error.
    } finally {
      setBusy(false)
    }
  }
  const isDanger = danger || okButtonProps?.danger
  return (
    <Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) onCancel?.() }}>
      <AlertDialogTrigger asChild>{children}</AlertDialogTrigger>
      {/* suppress Radix's missing-description warning when intentionally absent */}
      <AlertDialogContent {...(description == null ? { 'aria-describedby': undefined } : {})}>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          {description != null && <AlertDialogDescription>{description}</AlertDialogDescription>}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy} onClick={() => onCancel?.()}>{cancelText}</AlertDialogCancel>
          {/* a plain Button (not AlertDialogAction) so the dialog only closes on success. */}
          <Button variant={isDanger ? 'destructive' : 'default'} disabled={okButtonProps?.disabled} loading={busy} onClick={run}>{okText}</Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </Root>
  )
}
