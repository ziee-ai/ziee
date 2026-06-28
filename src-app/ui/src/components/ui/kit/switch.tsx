import * as React from 'react'
import { Loader2 } from 'lucide-react'
import { Switch as Base } from '../shadcn/switch'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { cn } from '@/lib/utils'

export interface SwitchProps {
  checked?: boolean
  /** Alias of `checked` so FormField's default valuePropName='value' also binds. */
  value?: boolean
  defaultChecked?: boolean
  onCheckedChange?: (checked: boolean) => void
  /** Alias of onCheckedChange for FormField binding (valuePropName="checked"). */
  onChange?: (checked: boolean) => void
  onBlur?: () => void
  disabled?: boolean
  /** Own busy state (legacy `loading`): disables + shows a spinner. NOT the region skeleton. */
  loading?: boolean
  size?: 'sm' | 'default'
  name?: string
  id?: string
  /** Inline label; if omitted, provide aria-label/aria-labelledby or wrap in a FormField. */
  label?: React.ReactNode
  className?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  'aria-describedby'?: string
  invalid?: boolean
  /** Test selector — forwarded onto <root> (i18n-safe). */
  'data-testid': string
}

export const Switch = React.forwardRef<HTMLButtonElement, SwitchProps>(function Switch(
  { checked, value, defaultChecked, onCheckedChange, onChange, onBlur, disabled, loading, size, name, id, label, className,
    'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledby, 'aria-describedby': ariaDescribedby, invalid,
    'data-testid': testid },
  ref,
) {
  const s = useSurface({ disabled })
  const reactId = React.useId()
  const ctrlId = id ?? reactId
  // compose: both the form binding (onChange) and any consumer canonical handler fire.
  const handle = (v: boolean) => {
    onCheckedChange?.(v)
    onChange?.(v)
  }
  if (s.loading) return <Skeleton className={cn('h-5 w-9 rounded-full', className)} />
  const baseEl = (
    <Base
      ref={ref}
      id={ctrlId}
      checked={checked ?? value}
      defaultChecked={defaultChecked}
      onCheckedChange={handle}
      onBlur={onBlur}
      disabled={s.disabled || s.readOnly || loading}
      name={name}
      aria-label={label == null ? ariaLabel : undefined}
      aria-labelledby={ariaLabelledby}
      aria-describedby={ariaDescribedby}
      aria-invalid={invalid || undefined}
      aria-busy={loading || undefined}
      data-testid={testid}
      className={cn(size === 'sm' && 'h-4 w-7 [&>span]:size-3 [&>span]:data-[state=checked]:translate-x-3', className)}
    />
  )
  // own loading → spinner overlay on the track (not a skeleton).
  const control = loading ? (
    <span className="relative inline-flex">
      {baseEl}
      <Loader2 className="pointer-events-none absolute left-1/2 top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 animate-spin opacity-80" aria-hidden />
    </span>
  ) : baseEl
  if (label == null) return control
  return (
    <div className="flex items-center gap-2">
      {control}
      <label htmlFor={ctrlId} className={cn('text-sm', s.disabled && 'opacity-60')}>{label}</label>
    </div>
  )
})
