import * as React from 'react'
import { format as formatDate, parseISO, isValid } from 'date-fns'
import { CalendarIcon } from 'lucide-react'
import { Popover as Root, PopoverTrigger, PopoverContent } from '../shadcn/popover'
import { Calendar } from '../shadcn/calendar'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { useControllableState } from './use-controllable-state'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

// Coerce a Date | ISO-string | '' into a Date (or undefined). String values come from
// FormField bindings / JSON-schema forms; a Date comes from direct programmatic use.
function toDate(v: Date | string | undefined): Date | undefined {
  if (v == null || v === '') return undefined
  if (v instanceof Date) return isValid(v) ? v : undefined
  const d = parseISO(v)
  return isValid(d) ? d : undefined
}

// Single-date picker: a Button trigger + a Popover-hosted shadcn Calendar.
// Form-bindable: value + onChange (alias onValueChange) + name (hidden input) + id + ref(→trigger).
// `value` accepts a Date or an ISO string; the change handlers EMIT a string in `valueFormat`
// (default ISO `yyyy-MM-dd`) so RHF / JSON-schema forms get a stable serializable value.
export type DatePickerProps = {
  value?: Date | string
  defaultValue?: Date | string
  /** Emits the picked date as a string in `valueFormat` (or '' when cleared). */
  onChange?: (value: string) => void
  /** Alias of onChange (parallels onValueChange elsewhere in the kit). */
  onValueChange?: (value: string) => void
  onBlur?: () => void
  /** Trigger text when nothing is selected (required — caller owns it for i18n). */
  placeholder: string
  /** date-fns display format for the trigger label. Default 'PP' (e.g. Apr 29, 2024). */
  format?: string
  /** date-fns format the change handlers emit. Default 'yyyy-MM-dd' (ISO date). */
  valueFormat?: string
  disabled?: boolean
  loading?: boolean
  invalid?: boolean
  size?: 'sm' | 'default' | 'lg'
  name?: string
  id?: string
  className?: string
  /** Accessible name for the trigger — REQUIRED (no default, for i18n). */
  'aria-label': string
  'aria-describedby'?: string
  'aria-labelledby'?: string
  'aria-required'?: boolean
} & KitStyleProps

const triggerH = (size?: 'sm' | 'default' | 'lg') =>
  size === 'sm' ? 'h-8 text-xs' : size === 'lg' ? 'h-10' : 'h-9'

export const DatePicker = React.forwardRef<HTMLButtonElement, DatePickerProps>(function DatePicker(
  {
    value, defaultValue, onChange, onValueChange, onBlur, placeholder,
    format = 'PP', valueFormat = 'yyyy-MM-dd',
    disabled, loading, invalid, size, name, id, className, style,
    'aria-label': ariaLabel, 'aria-describedby': ariaDescribedby,
    'aria-labelledby': ariaLabelledby, 'aria-required': ariaRequired,
  },
  ref,
) {
  const s = useSurface({ disabled, size })
  const [open, setOpen] = React.useState(false)
  // Single source of truth: a string ('' = no selection) so the value stays serializable.
  const [current, setCurrent] = useControllableState<string>({
    value: value === undefined ? undefined : (toDate(value) ? formatDate(toDate(value)!, valueFormat) : ''),
    defaultValue: toDate(defaultValue) ? formatDate(toDate(defaultValue)!, valueFormat) : '',
    onChange: (v) => { onChange?.(v); onValueChange?.(v) },
  })
  const locked = s.disabled || loading || s.readOnly
  const selected = toDate(current)

  if (s.loading) return <Skeleton className={cn(triggerH(s.size), 'w-full rounded-md', className)} />

  const choose = (d: Date | undefined) => {
    setCurrent(d ? formatDate(d, valueFormat) : '')
    setOpen(false)
  }

  return (
    <Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) onBlur?.() }}>
      {/* native form submission (the trigger is a button with no value of its own). */}
      {name != null && <input type="hidden" name={name} value={current} />}
      <PopoverTrigger asChild>
        <button
          ref={ref}
          type="button"
          id={id}
          aria-label={ariaLabel}
          aria-describedby={ariaDescribedby}
          aria-labelledby={ariaLabelledby}
          aria-required={ariaRequired || undefined}
          aria-invalid={invalid || undefined}
          aria-haspopup="dialog"
          aria-expanded={open}
          disabled={locked}
          style={style}
          className={cn(
            'flex w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-2 text-sm',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
            triggerH(s.size), className, invalid && 'border-destructive focus-visible:ring-destructive',
          )}
        >
          <span className={cn('truncate', !selected && 'text-muted-foreground')}>
            {selected ? formatDate(selected, format) : placeholder}
          </span>
          <CalendarIcon className="size-4 shrink-0 opacity-50" aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start">
        <Calendar
          mode="single"
          autoFocus
          selected={selected}
          onSelect={choose}
        />
      </PopoverContent>
    </Root>
  )
})
