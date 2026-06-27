import * as React from 'react'
import { Check, ChevronsUpDown } from 'lucide-react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Popover as Root, PopoverTrigger, PopoverContent } from '../shadcn/popover'
import { Command, CommandInput, CommandList, CommandEmpty, CommandGroup, CommandItem } from '../shadcn/command'
import { Skeleton } from '../shadcn/skeleton'
import { Tag } from './tag'
import { useSurface } from './surface'
import { useControllableState } from './use-controllable-state'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export interface MultiSelectOption {
  label: string
  value: string
  disabled?: boolean
}

// Virtualized multi-select listbox for large option sets (cmdk renders all rows + can't window).
// Own filter + arrow-key aria-activedescendant nav; toggling keeps the popover open.
function VirtualMultiList({
  options, selectedSet, onToggle, searchPlaceholder, emptyText, listboxId,
}: {
  options: MultiSelectOption[]
  selectedSet: Set<string>
  onToggle: (value: string) => void
  searchPlaceholder: string
  emptyText: string
  listboxId: string
}) {
  const [query, setQuery] = React.useState('')
  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return q ? options.filter((o) => o.label.toLowerCase().includes(q)) : options
  }, [options, query])
  const firstEnabled = React.useMemo(() => filtered.findIndex((o) => !o.disabled), [filtered])
  const lastEnabled = React.useMemo(() => { for (let i = filtered.length - 1; i >= 0; i--) if (!filtered[i].disabled) return i; return -1 }, [filtered])
  const [active, setActive] = React.useState(0)
  React.useEffect(() => { setActive(firstEnabled >= 0 ? firstEnabled : 0) }, [firstEnabled])
  const scrollRef = React.useRef<HTMLDivElement>(null)
  const inputRef = React.useRef<HTMLInputElement>(null)
  React.useEffect(() => { inputRef.current?.focus() }, [])
  const virtualizer = useVirtualizer({
    count: filtered.length, getScrollElement: () => scrollRef.current, estimateSize: () => 32, overscan: 8,
  })
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
    else if (e.key === 'Enter') { e.preventDefault(); const o = filtered[active]; if (o && !o.disabled) onToggle(o.value) }
  }
  const virtualItems = virtualizer.getVirtualItems()
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
        <div ref={scrollRef} role="listbox" aria-multiselectable id={listboxId} className="max-h-72 overflow-auto p-1">
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualItems.map((vi) => {
              const o = filtered[vi.index]
              const selected = selectedSet.has(o.value)
              const isActive = active === vi.index
              return (
                <div
                  key={o.value}
                  id={`${listboxId}-${vi.index}`}
                  role="option"
                  aria-selected={selected}
                  aria-disabled={o.disabled || undefined}
                  onMouseMove={() => setActive(vi.index)}
                  onClick={() => { if (!o.disabled) onToggle(o.value) }}
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

// Multi-select with searchable list + removable tags (legacy Select mode="multiple").
// Form-bindable: value:string[] + onChange(string[]) + name + id + ref.
export type MultiSelectProps = {
  options: MultiSelectOption[]
  value?: string[]
  defaultValue?: string[]
  onValueChange?: (value: string[]) => void
  /** Alias of onValueChange for FormField binding. */
  onChange?: (value: string[]) => void
  onBlur?: () => void
  placeholder: string
  searchPlaceholder: string
  emptyText: string
  /** Builds the accessible name for a tag's remove button, e.g. (label) => `Remove ${label}`. */
  removeLabel: (label: string) => string
  disabled?: boolean
  loading?: boolean
  invalid?: boolean
  /** Window the option list for large sets (own filter + keyboard nav, replaces cmdk). */
  virtual?: boolean
  name?: string
  id?: string
  className?: string
  'aria-describedby'?: string
  'aria-label'?: string
  'aria-labelledby'?: string
  'aria-required'?: boolean
} & KitStyleProps

export const MultiSelect = React.forwardRef<HTMLDivElement, MultiSelectProps>(function MultiSelect(
  { options, value, defaultValue, onValueChange, onChange, onBlur, placeholder, searchPlaceholder, emptyText, removeLabel,
    disabled, loading, invalid, virtual, name, id, className, style,
    'aria-describedby': ariaDescribedby, 'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledby,
    'aria-required': ariaRequired },
  ref,
) {
  const s = useSurface({ disabled })
  const listboxId = React.useId()
  const [open, setOpen] = React.useState(false)
  const [current, setCurrent] = useControllableState<string[]>({
    value, defaultValue: defaultValue ?? [], onChange: (v) => { onValueChange?.(v); onChange?.(v) },
  })
  const locked = s.disabled || loading || s.readOnly
  // O(1) lookups: label-by-value (tag render) + a selected Set (Check + toggle) — replaces the
  // per-item `.find()`/`.includes()` that made selection rendering O(n²).
  const labelByValue = React.useMemo(() => {
    const m = new Map<string, string>()
    for (const o of options) m.set(o.value, o.label)
    return m
  }, [options])
  const selectedSet = React.useMemo(() => new Set(current), [current])
  // functional updater (the hook supports it) → no stale read-modify-write on rapid toggles.
  const toggle = (v: string) => setCurrent((prev) => prev.includes(v) ? prev.filter((x) => x !== v) : [...prev, v])
  if (s.loading) return <Skeleton className={cn('h-9 w-full rounded-md', className)} />
  return (
    <Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) onBlur?.() }}>
      {/* native form submission: one hidden input per selected value (div trigger has no name). */}
      {name != null && current.map((v) => <input key={v} type="hidden" name={name} value={v} />)}
      <PopoverTrigger asChild>
        {/* a DIV (not a <button>) so the removable Tag <button>s can legally nest; keyboard
            open is wired manually since a div has no implicit button activation. */}
        <div
          ref={ref}
          id={id}
          role="combobox"
          tabIndex={locked ? -1 : 0}
          aria-expanded={open}
          aria-haspopup="listbox"
          aria-controls={listboxId}
          aria-invalid={invalid || undefined}
          aria-busy={loading || undefined}
          aria-describedby={ariaDescribedby}
          aria-label={ariaLabel}
          aria-labelledby={ariaLabelledby}
          aria-required={ariaRequired || undefined}
          aria-disabled={locked || undefined}
          style={style}
          onKeyDown={(e) => {
            if (locked) return
            if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') { e.preventDefault(); setOpen(true) }
          }}
          className={cn(
            'flex min-h-9 w-full flex-wrap items-center gap-1 rounded-md border border-input bg-transparent px-2 py-1 text-sm',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
            locked && 'cursor-not-allowed opacity-50',
            className, invalid && 'border-destructive focus-visible:ring-destructive',
          )}
        >
          {current.length === 0 && <span className="px-1 text-muted-foreground">{placeholder}</span>}
          {current.map((v) => {
            const label = labelByValue.get(v) ?? v
            return (
              // stop the remove click/keys from bubbling to the trigger (which would open it).
              <span key={v} onClick={(e) => e.stopPropagation()} onPointerDown={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                <Tag onClose={() => { if (!locked) toggle(v) }} closeLabel={removeLabel(label)}>
                  {label}
                </Tag>
              </span>
            )
          })}
          <ChevronsUpDown className="ml-auto size-4 shrink-0 opacity-50" aria-hidden />
        </div>
      </PopoverTrigger>
      <PopoverContent className="w-[--radix-popover-trigger-width] p-0" align="start">
        {virtual ? (
          <VirtualMultiList options={options} selectedSet={selectedSet} onToggle={toggle} searchPlaceholder={searchPlaceholder} emptyText={emptyText} listboxId={listboxId} />
        ) : (
          <Command>
            <CommandInput placeholder={searchPlaceholder} />
            <CommandList id={listboxId}>
              <CommandEmpty>{emptyText}</CommandEmpty>
              <CommandGroup>
                {options.map((o) => (
                  <CommandItem key={o.value} value={o.value} keywords={[o.label]} disabled={o.disabled} onSelect={() => toggle(o.value)}>
                    <Check className={cn('mr-2 size-4', selectedSet.has(o.value) ? 'opacity-100' : 'opacity-0')} aria-hidden />
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
