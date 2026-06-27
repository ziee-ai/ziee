import * as React from 'react'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { type KitStyleProps } from './style-guard'

// legacy Tag → kit Tag. Semantic tones (NOT arbitrary hex). Optional close button is
// a real <button> with a forced accessible name.
export type TagTone = 'default' | 'primary' | 'success' | 'warning' | 'error' | 'info'
/** Fill style. soft (default) = tinted bg; solid = saturated bg (legacy `filled`);
 *  outline = transparent bg + colored border (legacy `outlined`/`borderless`). */
export type TagVariant = 'soft' | 'solid' | 'outline'

const tones: Record<TagTone, string> = {
  default: 'bg-muted text-foreground/80 border-transparent',
  primary: 'bg-primary/10 text-primary border-primary/20',
  success: 'bg-green-500/10 text-green-700 dark:text-green-400 border-green-500/20',
  warning: 'bg-amber-500/10 text-amber-700 dark:text-amber-400 border-amber-500/20',
  error: 'bg-destructive/10 text-destructive border-destructive/20',
  info: 'bg-blue-500/10 text-blue-700 dark:text-blue-400 border-blue-500/20',
}
const solidTones: Record<TagTone, string> = {
  default: 'bg-foreground/80 text-background border-transparent',
  primary: 'bg-primary text-primary-foreground border-transparent',
  success: 'bg-green-600 text-white border-transparent',
  warning: 'bg-amber-500 text-white border-transparent',
  error: 'bg-destructive text-white border-transparent',
  info: 'bg-blue-600 text-white border-transparent',
}
const outlineTones: Record<TagTone, string> = {
  default: 'bg-transparent text-foreground/80 border-border',
  primary: 'bg-transparent text-primary border-primary/40',
  success: 'bg-transparent text-green-700 dark:text-green-400 border-green-500/40',
  warning: 'bg-transparent text-amber-700 dark:text-amber-400 border-amber-500/40',
  error: 'bg-transparent text-destructive border-destructive/40',
  info: 'bg-transparent text-blue-700 dark:text-blue-400 border-blue-500/40',
}
const variantTones: Record<TagVariant, Record<TagTone, string>> = {
  soft: tones, solid: solidTones, outline: outlineTones,
}

type TagBase = Omit<React.ComponentProps<'span'>, 'ref' | 'style'> & {
  tone?: TagTone
  variant?: TagVariant
  icon?: React.ReactNode
} & KitStyleProps
// A closable Tag MUST supply both onClose and an explicit closeLabel (no default — the
// caller owns the string for i18n).
export type TagProps =
  | (TagBase & { onClose?: undefined; closeLabel?: never })
  | (TagBase & { onClose: () => void; closeLabel: string })

// React.memo: Tags are rendered in large lists (MultiSelect tags, table cells) where the parent
// re-renders for unrelated reasons — memo skips unchanged tags.
const TagInner = React.forwardRef<HTMLSpanElement, TagProps>(function Tag(
  { tone = 'default', variant = 'soft', icon, className, children, ...rest }, ref,
) {
  const onClose = (rest as { onClose?: () => void }).onClose
  const closeLabel = (rest as { closeLabel?: string }).closeLabel
  const props = (() => {
    // strip kit-only props so they don't leak onto the DOM span.
    const { onClose: _o, closeLabel: _c, allowStyle: _a, ...p } = rest as Record<string, unknown>
    return p
  })()
  return (
    <span
      ref={ref}
      className={cn(
        'inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium',
        variantTones[variant][tone],
        className,
      )}
      {...props}
    >
      {icon != null && <span className="[&_svg]:size-3" aria-hidden>{icon}</span>}
      {children}
      {onClose != null && (
        <button
          type="button"
          onClick={onClose}
          aria-label={closeLabel}
          className="-mr-0.5 ml-0.5 inline-flex items-center justify-center rounded-sm opacity-60 hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <X className="size-3" aria-hidden />
        </button>
      )}
    </span>
  )
})
export const Tag = React.memo(TagInner)
