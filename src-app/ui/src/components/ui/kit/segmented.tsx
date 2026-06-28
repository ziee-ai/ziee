import * as React from 'react'
import { ToggleGroup, ToggleGroupItem } from '../shadcn/toggle-group'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { useControllableState } from './use-controllable-state'
import { cn } from '@/lib/utils'

export interface SegmentedOption {
  label: React.ReactNode
  value: string
  disabled?: boolean
}

export interface SegmentedProps {
  options: SegmentedOption[]
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  /** Alias of onValueChange for FormField binding. */
  onChange?: (value: string) => void
  onBlur?: () => void
  disabled?: boolean
  size?: 'sm' | 'default' | 'lg'
  name?: string
  id?: string
  invalid?: boolean
  className?: string
  'aria-label'?: string
  'aria-describedby'?: string
  /** Test selector — forwarded onto <root> (i18n-safe). */
  'data-testid'?: string
}

const itemH = (size?: 'sm' | 'default' | 'lg') => (size === 'sm' ? 'h-7 text-xs' : size === 'lg' ? 'h-10' : 'h-8')

export const Segmented = React.forwardRef<HTMLDivElement, SegmentedProps>(function Segmented(
  { options, value, defaultValue, onValueChange, onChange, onBlur, disabled, size, name, id, invalid, className,
    'aria-label': ariaLabel, 'aria-describedby': ariaDescribedby, 'data-testid': testid },
  ref,
) {
  const s = useSurface({ disabled, size })
  // Merged controlled/uncontrolled selection (keeps the hidden form input correct in both modes).
  const [current, setCurrent] = useControllableState<string>({
    value, defaultValue: defaultValue ?? '', onChange: (v) => { onValueChange?.(v); onChange?.(v) },
  })
  const handle = (v: string) => {
    if (!v) return // ToggleGroup emits '' on deselect — keep a value selected
    setCurrent(v)
  }
  if (s.loading) return <Skeleton className={cn(itemH(s.size), 'w-full rounded-md', className)} />
  return (
    <>
    {/* carry `name` for native form submission / name-based selectors (ToggleGroup has none). */}
    {name != null && <input type="hidden" name={name} value={current} />}
    <ToggleGroup
      ref={ref}
      id={id}
      type="single"
      value={current}
      onValueChange={handle}
      onBlur={onBlur}
      disabled={s.disabled || s.readOnly}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedby}
      aria-invalid={invalid || undefined}
      data-testid={testid}
      className={cn('inline-flex rounded-md bg-muted p-0.5', className)}
    >
      {options.map((o) => (
        <ToggleGroupItem
          key={o.value}
          value={o.value}
          disabled={o.disabled}
          className={cn(itemH(s.size), 'data-[state=on]:bg-background data-[state=on]:shadow-sm')}
        >
          {o.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
    </>
  )
})
