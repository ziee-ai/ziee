import * as React from 'react'
import { CheckCircle2, Info, AlertTriangle, XCircle, X } from 'lucide-react'
import { Alert as Base, AlertTitle, AlertDescription } from '../shadcn/alert'
import { cn } from '@/lib/utils'

export type AlertTone = 'info' | 'success' | 'warning' | 'error'

const toneIcon: Record<AlertTone, React.ComponentType<{ className?: string }>> = {
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  error: XCircle,
}
const toneCls: Record<AlertTone, string> = {
  info: 'border-blue-500/30 text-blue-800 dark:text-blue-300 [&>svg]:text-blue-600',
  success: 'border-green-500/30 text-green-800 dark:text-green-300 [&>svg]:text-green-600',
  warning: 'border-amber-500/30 text-amber-900 dark:text-amber-300 [&>svg]:text-amber-600',
  error: 'border-destructive/40 text-destructive [&>svg]:text-destructive',
}

interface AlertCommon {
  tone?: AlertTone
  title?: React.ReactNode
  description?: React.ReactNode
  icon?: React.ReactNode
  className?: string
  children?: React.ReactNode
}
// A dismissible alert MUST supply both onClose and an explicit closeLabel (no built-in
// default — the caller owns the string so it can be translated).
export type AlertProps =
  | (AlertCommon & { onClose?: undefined; closeLabel?: never })
  | (AlertCommon & { onClose: () => void; closeLabel: string })

export function Alert({ tone = 'info', title, description, icon, className, children, ...rest }: AlertProps) {
  const Icon = toneIcon[tone]
  const role = tone === 'error' || tone === 'warning' ? 'alert' : 'status'
  const onClose = (rest as { onClose?: () => void }).onClose
  const closeLabel = (rest as { closeLabel?: string }).closeLabel
  return (
    <Base role={role} className={cn(toneCls[tone], onClose && 'pr-10', 'relative', className)}>
      {icon != null ? <span aria-hidden>{icon}</span> : <Icon className="size-4" aria-hidden />}
      {title != null && <AlertTitle>{title}</AlertTitle>}
      {(description != null || children != null) && (
        <AlertDescription>{description}{children}</AlertDescription>
      )}
      {onClose && (
        <button
          type="button"
          onClick={onClose}
          aria-label={closeLabel}
          className="absolute right-2 top-2 rounded-sm p-1 text-foreground/60 hover:text-foreground hover:bg-foreground/10"
        >
          <X className="size-4" aria-hidden />
        </button>
      )}
    </Base>
  )
}
