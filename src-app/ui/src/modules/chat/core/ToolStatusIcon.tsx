import { cn } from '@/lib/utils'
import {
  TOOL_STATUS,
  toolStatusKey,
  type ToolStatusKey,
} from './tool-status'

/**
 * Renders the status icon for a tool call from the single {@link TOOL_STATUS}
 * source. Accepts either a canonical {@link ToolStatusKey} or a raw status
 * string (+ optional `isError`), which it normalizes via {@link toolStatusKey}.
 *
 * Sizing (Spec A / finding #16): the icon is `size-4` (16px) — ~1.1× the cap
 * height of the adjacent `text-sm` label and comfortably inside the 1.0–1.2×
 * line-height band — and `shrink-0` so it never distorts. Pair it with a label
 * inside {@link ToolStatusInline} (or any `inline-flex items-center` container)
 * so the icon and label stay one non-wrapping unit instead of the icon ballooning
 * to lucide's 24px default and breaking onto its own line.
 */
export function ToolStatusIcon({
  status,
  isError,
  className,
}: {
  status: ToolStatusKey | string
  isError?: boolean
  className?: string
}) {
  const key = (status in TOOL_STATUS ? status : toolStatusKey(status, isError)) as ToolStatusKey
  const d = TOOL_STATUS[key]
  const Icon = d.icon
  return (
    <Icon
      aria-hidden
      className={cn('size-4 shrink-0', d.color, d.spin && 'animate-spin', className)}
    />
  )
}

/**
 * Icon + label as ONE inline, non-wrapping unit (`inline-flex` + `whitespace-nowrap`).
 * The icon never separates from its text and the icon stays `shrink-0`; the label
 * truncates before the pair wraps. Use wherever a status needs a visible label
 * (e.g. the approval card header).
 */
export function ToolStatusInline({
  status,
  isError,
  label,
  className,
}: {
  status: ToolStatusKey | string
  isError?: boolean
  /** Override the default {@link TOOL_STATUS} label. */
  label?: React.ReactNode
  className?: string
}) {
  const key = (status in TOOL_STATUS ? status : toolStatusKey(status, isError)) as ToolStatusKey
  const d = TOOL_STATUS[key]
  return (
    <span className={cn('inline-flex items-center gap-1.5 whitespace-nowrap', className)}>
      <ToolStatusIcon status={key} />
      <span className="min-w-0 truncate">{label ?? d.label}</span>
    </span>
  )
}
