import * as React from 'react'
import { Eye, EyeOff, Loader2 } from 'lucide-react'
import { Input as InputBase } from '../shadcn/input'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

// Native <input> + kit additions (prefix/suffix adornments, invalid, density).
// `size` is density ('sm'|'default'|'lg'), NOT the native numeric size.
// Surface: region loading → skeleton · own `loading` → suffix spinner (+ disabled) · disabled · readOnly · size.
export type InputProps = Omit<React.ComponentProps<'input'>, 'size' | 'prefix' | 'style'> & {
  size?: 'sm' | 'default' | 'lg'
  loading?: boolean
  prefix?: React.ReactNode
  suffix?: React.ReactNode
  invalid?: boolean
} & KitStyleProps

const heightFor = (size?: 'sm' | 'default' | 'lg') =>
  size === 'sm' ? 'h-8 text-xs' : size === 'lg' ? 'h-10' : 'h-9'

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ size: ownSize, loading, prefix, suffix, invalid, disabled, readOnly, style, allowStyle, className, ...props }, ref) => {
    const s = useSurface({ disabled, readOnly, size: ownSize })

    if (s.loading) {
      return <Skeleton className={cn(heightFor(s.size), 'w-full rounded-md', className)} />
    }

    const rightAdornment = loading ? <Loader2 className="size-4 animate-spin opacity-70" aria-hidden /> : suffix
    const field = (
      <InputBase
        ref={ref}
        style={style}
        disabled={s.disabled || loading}
        readOnly={s.readOnly}
        aria-invalid={invalid || undefined}
        aria-busy={loading || undefined}
        className={cn(
          heightFor(s.size),
          prefix && 'pl-9',
          rightAdornment && 'pr-9',
          className,
          invalid && 'border-destructive focus-visible:ring-destructive',
        )}
        {...props}
      />
    )
    if (!prefix && !rightAdornment) return field
    return (
      <div className="relative w-full">
        {prefix && (
          <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground [&_svg]:size-4">
            {prefix}
          </span>
        )}
        {field}
        {rightAdornment && (
          // non-interactive by default; interactive adornments (password toggle) opt back in.
          <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground [&_svg]:size-4">
            {rightAdornment}
          </span>
        )}
      </div>
    )
  },
)
Input.displayName = 'Input'

// shadcn has no password input — kit addition with a keyboard-accessible show/hide toggle.
// showLabel/hideLabel are REQUIRED (no default) so the toggle's accessible name is always
// caller-owned and translatable.
export type PasswordInputProps = Omit<InputProps, 'type' | 'suffix' | 'style' | 'allowStyle'> & {
  showLabel: string
  hideLabel: string
}
export const PasswordInput = React.forwardRef<HTMLInputElement, PasswordInputProps>(
  ({ showLabel, hideLabel, ...props }, ref) => {
    const [show, setShow] = React.useState(false)
    return (
      <Input
        {...props}
        ref={ref}
        type={show ? 'text' : 'password'}
        suffix={
          <button
            type="button"
            onClick={() => setShow((v) => !v)}
            className="pointer-events-auto text-muted-foreground hover:text-foreground"
            aria-label={show ? hideLabel : showLabel}
            aria-pressed={show}
          >
            {show ? <EyeOff aria-hidden /> : <Eye aria-hidden />}
          </button>
        }
      />
    )
  },
)
PasswordInput.displayName = 'PasswordInput'
