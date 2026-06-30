import * as React from 'react'
import { Check, ChevronsUpDown, Loader2 } from 'lucide-react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Popover as Root, PopoverTrigger, PopoverContent } from '../shadcn/popover'
import { Command, CommandInput, CommandList, CommandEmpty, CommandGroup, CommandItem } from '../shadcn/command'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { useControllableState } from './use-controllable-state'
import { type KitStyleProps } from './style-guard'
import type { ValueBinding } from './value-binding'
import { cn } from '@/lib/utils'

export interface ComboboxOption {
  label: string
  value: string
  disabled?: boolean
}

// Virtualized listbox for large option sets (cmdk renders all rows + can't window). Owns its own
// filter + arrow-key navigation (aria-activedescendant) + scroll-into-view. Mounted fresh on open.
function VirtualList({
  options, current, onChoose, searchPlaceholder, emptyText, listboxId,
  optionTestid,
}: {
  options: ComboboxOption[]
  current: string | undefined
  onChoose: (value: string) => void
  searchPlaceholder: string
  emptyText: string
  listboxId: string
  optionTestid?: (value: string) => string | undefined
}) {
  const [query, setQuery] = React.useState('')
  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return q ? options.filter((o) => o.label.toLowerCase().includes(q)) : options
  }, [options, query])
  const firstEnabled = React.useMemo(() => filtered.findIndex((o) => !o.disabled), [filtered])
  const lastEnabled = React.useMemo(() => { for (let i = filtered.length - 1; i >= 0; i--) if (!filtered[i].disabled) return i; return -1 }, [filtered])
  const [active, setActive] = React.useState(0)
  // reset active to the first ENABLED option whenever the filtered set changes.
  React.useEffect(() => { setActive(firstEnabled >= 0 ? firstEnabled : 0) }, [firstEnabled])
  const scrollRef = React.useRef<HTMLDivElement>(null)
  const inputRef = React.useRef<HTMLInputElement>(null)
  React.useEffect(() => { inputRef.current?.focus() }, [])
  const virtualizer = useVirtualizer({
    count: filtered.length, getScrollElement: () => scrollRef.current, estimateSize: () => 32, overscan: 8,
  })
  // step to the next ENABLED option in a direction (skips disabled, like the default path).
  const nextEnabled = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < filtered.length; i += dir) if (!filtered[i].disabled) return i
    return -1
  }
  const goTo = (n: number) => { if (n >= 0) { setActive(n); virtualizer.scrollToIndex(n) } }
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); goTo(nextEnabled(active, 1)) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); goTo(nextEnabled(active, -1)) }
    else if (e.key === 'Home') { e.preventDefault(); goTo(firstEnabled) }
    else if (e.key === 'End') { e.preventDefault(); goTo(lastEnabled) }
    else if (e.key === 'Enter') { e.preventDefault(); const o = filtered[active]; if (o && !o.disabled) onChoose(o.value) }
  }
  const virtualItems = virtualizer.getVirtualItems()
  // aria-activedescendant must reference a MOUNTED option (windowing unmounts off-screen rows).
  const activeMounted = virtualItems.some((vi) => vi.index === active)
  return (
    <div>
      <div className="flex items-center border-b px-3">
        <input
          ref={inputRef}
          role="combobox"
          aria-expanded
          aria-controls={listboxId}
          aria-autocomplete="list"
          aria-activedescendant={activeMounted && filtered[active] ? `${listboxId}-${active}` : undefined}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={searchPlaceholder}
          className="h-10 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
        />
      </div>
      {filtered.length === 0 ? (
        <div className="py-6 text-center text-sm text-muted-foreground">{emptyText}</div>
      ) : (
        <div ref={scrollRef} role="listbox" id={listboxId} className="max-h-72 overflow-auto p-1">
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualItems.map((vi) => {
              const o = filtered[vi.index]
              const selected = current === o.value
              const isActive = active === vi.index
              return (
                <div
                  key={o.value}
                  id={`${listboxId}-${vi.index}`}
                  data-testid={optionTestid?.(o.value)}
                  role="option"
                  aria-selected={selected}
                  aria-disabled={o.disabled || undefined}
                  onMouseMove={() => setActive(vi.index)}
                  onClick={() => { if (!o.disabled) onChoose(o.value) }}
                  style={{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${vi.start}px)` }}
                  className={cn(
                    'flex h-8 cursor-pointer items-center gap-2 rounded-sm px-2 text-sm',
                    isActive && 'bg-accent',
                    o.disabled && 'pointer-events-none opacity-50',
                  )}
                >
                  <Check className={cn('size-4 shrink-0', selected ? 'opacity-100' : 'opacity-0')} aria-hidden />
                  <span className="truncate">{o.label}</span>
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}

// Searchable single-select (legacy Select showSearch). Radix Select can't filter, so this is
// a Popover + Command (cmdk). Form-bindable: value + onChange + onBlur + name + id + ref.
interface ComboboxBase {
  options: ComboboxOption[]
  onBlur?: () => void
  /** Trigger text when nothing is selected (required — caller owns it for i18n). */
  placeholder: string
  /** Search box placeholder (required — i18n). */
  searchPlaceholder: string
  /** Shown when the filter matches nothing (required — i18n). */
  emptyText: string
  disabled?: boolean
  loading?: boolean
  invalid?: boolean
  /** Window the option list for large sets (own filter + keyboard nav, replaces cmdk). */
  virtual?: boolean
  size?: 'sm' | 'default' | 'lg'
  name?: string
  id?: string
  className?: string
  'aria-describedby'?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  /** Test selector — forwarded onto <root> (i18n-safe). Options derive `${testid}-opt-${value}`. */
  'data-testid': string
}
// Controlled `value` requires a change handler (see ValueBinding); FormField stays valid.
export type ComboboxProps = ComboboxBase & ValueBinding<string> & KitStyleProps

export const Combobox = React.forwardRef<HTMLButtonElement, ComboboxProps>(function Combobox(
  { options, value, defaultValue, onValueChange, onChange, onBlur, placeholder, searchPlaceholder, emptyText,
    disabled, loading, invalid, virtual, size, name, id, className, style,
    'aria-describedby': ariaDescribedby, 'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledby,
    'data-testid': testid },
  ref,
) {
  const s = useSurface({ disabled, size })
  const optionTestid = React.useCallback(
    (v: string) => (testid ? `${testid}-opt-${v}` : undefined),
    [testid],
  )
  const listboxId = React.useId()
  const [open, setOpen] = React.useState(false)
  const [current, setCurrent] = useControllableState<string>({
    value, defaultValue: defaultValue ?? '', onChange: (v) => { onValueChange?.(v); onChange?.(v) },
  })
  const locked = s.disabled || loading || s.readOnly
  const choose = (v: string) => { setCurrent(v); setOpen(false) }
  if (s.loading) return <Skeleton className={cn('h-9', 'w-full rounded-md', className)} />
  const currentLabel = options.find((o) => o.value === current)?.label
  return (
    <Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) onBlur?.() }}>
      <PopoverTrigger asChild>
        <button
          ref={ref}
          type="button"
          id={id}
          name={name}
          aria-expanded={open}
          aria-haspopup="listbox"
          aria-controls={listboxId}
          aria-invalid={invalid || undefined}
          aria-busy={loading || undefined}
          aria-describedby={ariaDescribedby}
          aria-label={ariaLabel}
          aria-labelledby={ariaLabelledby}
          data-testid={testid}
          disabled={locked}
          style={style}
          className={cn(
            'flex w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-2 text-sm',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
            'h-9', className, invalid && 'border-destructive focus-visible:ring-destructive',
          )}
        >
          <span className={cn('truncate', currentLabel == null && 'text-muted-foreground')}>
            {currentLabel ?? placeholder}
          </span>
          {loading
            ? <Loader2 className="size-4 shrink-0 animate-spin opacity-70" aria-hidden />
            : <ChevronsUpDown className="size-4 shrink-0 opacity-50" aria-hidden />}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[--radix-popover-trigger-width] p-0" align="start">
        {virtual ? (
          <VirtualList options={options} current={current} onChoose={choose} searchPlaceholder={searchPlaceholder} emptyText={emptyText} listboxId={listboxId} optionTestid={optionTestid} />
        ) : (
          <Command>
            <CommandInput placeholder={searchPlaceholder} />
            <CommandList id={listboxId}>
              <CommandEmpty>{emptyText}</CommandEmpty>
              <CommandGroup>
                {options.map((o) => (
                  <CommandItem key={o.value} value={o.value} keywords={[o.label]} disabled={o.disabled} onSelect={() => choose(o.value)} data-testid={optionTestid(o.value)}>
                    <Check className={cn('mr-2 size-4', current === o.value ? 'opacity-100' : 'opacity-0')} aria-hidden />
                    {o.label}
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        )}
      </PopoverContent>
    </Root>
  )
})
