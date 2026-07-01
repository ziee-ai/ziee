import * as React from 'react'
import { Loader2 } from 'lucide-react'
import { Switch as Base } from '../shadcn/switch'
import { Skeleton } from '../shadcn/skeleton'
import { Tooltip } from './tooltip'
import { useSurface } from './surface'
import { cn } from '@/lib/utils'
import type { CheckedBinding } from './value-binding'

interface SwitchBase {
  onBlur?: () => void
  disabled?: boolean
  /** Own busy state (legacy `loading`): disables + shows a spinner. NOT the region skeleton. */
  loading?: boolean
  size?: 'sm' | 'default'
  name?: string
  id?: string
  /** Inline label; if omitted, provide aria-label/aria-labelledby or wrap in a FormField. */
  label?: React.ReactNode
  /** Tooltip on hover/focus. For a label-less switch it ALSO becomes the accessible
   * name (aria-label) when it's a string — so a bare switch is never nameless. */
  tooltip?: React.ReactNode
  className?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  'aria-describedby'?: string
  invalid?: boolean
  /** Test selector — forwarded onto <root> (i18n-safe). */
  'data-testid': string
}
// Controlled `checked` requires a change handler (see CheckedBinding); FormField stays valid.
export type SwitchProps = SwitchBase & CheckedBinding

export const Switch = React.forwardRef<HTMLButtonElement, SwitchProps>(function Switch(
  { checked, value, defaultChecked, onCheckedChange, onChange, onBlur, disabled, loading, size, name, id, label, tooltip, className,
    'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledby, 'aria-describedby': ariaDescribedby, invalid,
    'data-testid': testid },
  ref,
) {
  // A label-less switch with a string tooltip gets that tooltip as its accessible name.
  const nameFromTooltip = typeof tooltip === 'string' ? tooltip : undefined
  const s = useSurface({ disabled })
  const reactId = React.useId()
  const ctrlId = id ?? reactId
  // compose: both the form binding (onChange) and any consumer canonical handler fire.
  const handle = (v: boolean) => {
    onCheckedChange?.(v)
    onChange?.(v)
  }
  if (s.loading) return <Skeleton className={cn('h-[1.15rem] w-8 rounded-full', className)} />
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
      aria-label={label == null ? (ariaLabel ?? nameFromTooltip) : undefined}
      aria-labelledby={ariaLabelledby}
      aria-describedby={ariaDescribedby}
      aria-invalid={invalid || undefined}
      aria-busy={loading || undefined}
      data-testid={testid}
      size={size}
      className={className}
    />
  )
  // own loading → spinner overlay on the track (not a skeleton).
  const control = loading ? (
    <span className="relative inline-flex">
      {baseEl}
      <Loader2 className="pointer-events-none absolute left-1/2 top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 animate-spin opacity-80" aria-hidden />
    </span>
  ) : baseEl
  // Tooltip trigger is an inert wrapping <span>, NOT the switch itself — making
  // the tiny (~18px) interactive switch the trigger caused base-ui to open then
  // immediately close the tooltip (flicker). Same stable pattern as the kit
  // Tooltip used everywhere else.
  const maybeTip = tooltip != null ? (
    <Tooltip content={tooltip}>
      <span className="inline-flex">{control}</span>
    </Tooltip>
  ) : control
  if (label == null) return maybeTip
  return (
    <div className="flex items-center gap-2">
      {control}
      <label htmlFor={ctrlId} className={cn('text-sm', s.disabled && 'opacity-60')}>{label}</label>
    </div>
  )
})
