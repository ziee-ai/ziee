import {
  CheckCircle2,
  XCircle,
  CircleSlash,
  Loader2,
  Clock,
  AlarmClockOff,
  type LucideIcon,
} from 'lucide-react'

/**
 * Single source of truth for how a tool call's lifecycle status is presented —
 * icon, icon color, spin, badge tone, and label. Every chat tool-call card and
 * every status badge/chip renders through this map so the vocabulary can never
 * drift (the round-1 bug: a *cancelled* call rendered with the same red X /
 * error tone as a *failed* call, making a user-initiated cancel look like a
 * crash).
 *
 * Design rules (Spec A):
 * - `failed` OWNS the red XCircle. No other status uses that icon or the
 *   destructive red, so a red X unambiguously means "the tool errored".
 * - `cancelled` is NEUTRAL — a slashed circle in muted-foreground gray. A cancel
 *   is a user choice, not a failure, so it must never read as an error.
 * - `timeout` is amber/`warning` (a distinct AlarmClockOff icon), not red —
 *   red stays exclusive to `failed`.
 */
export type ToolStatusKey =
  | 'success'
  | 'failed'
  | 'cancelled'
  | 'running'
  | 'pending-approval'
  | 'timeout'

/** Subset of the kit `TagTone` union used by tool statuses (kept local so this
 *  pure module has no `@/` runtime import and stays unit-testable under
 *  `node --test`). Assignable to `TagTone` at every call site. */
export type ToolStatusTone = 'success' | 'error' | 'warning' | 'default' | 'primary'

export interface ToolStatusDescriptor {
  /** lucide icon for this status. */
  icon: LucideIcon
  /** Semantic text-color token class for the icon (the single source of truth
   *  for status color; `cancelled` MUST differ from `failed`). */
  color: string
  /** Whether the icon spins (in-flight states). */
  spin: boolean
  /** Badge/chip tone for text pills rendered from the same status. */
  tone: ToolStatusTone
  /** Human-readable label. */
  label: string
}

export const TOOL_STATUS: Record<ToolStatusKey, ToolStatusDescriptor> = {
  success: {
    icon: CheckCircle2,
    color: 'text-success',
    spin: false,
    tone: 'success',
    label: 'Completed',
  },
  failed: {
    icon: XCircle,
    color: 'text-destructive',
    spin: false,
    tone: 'error',
    label: 'Failed',
  },
  cancelled: {
    icon: CircleSlash,
    color: 'text-muted-foreground',
    spin: false,
    tone: 'default',
    label: 'Cancelled',
  },
  running: {
    icon: Loader2,
    color: 'text-primary',
    spin: true,
    tone: 'primary',
    label: 'Running',
  },
  'pending-approval': {
    icon: Clock,
    color: 'text-warning',
    spin: false,
    tone: 'warning',
    label: 'Pending approval',
  },
  timeout: {
    icon: AlarmClockOff,
    color: 'text-warning',
    spin: false,
    tone: 'warning',
    label: 'Timed out',
  },
}

/**
 * Normalize a raw tool-call status string (from any surface's vocabulary — the
 * live chat store's `started`/`completed`/`error`/`pending_approval`/`pending`,
 * or the persisted `mcp_tool_calls` terminal `completed`/`failed`/`timeout`/
 * `cancelled`) into a canonical {@link ToolStatusKey}. An explicit `isError`
 * flag (from a `tool_result` block) forces `failed`.
 */
export function toolStatusKey(raw: string | null | undefined, isError?: boolean): ToolStatusKey {
  if (isError) return 'failed'
  switch (raw) {
    case 'success':
    case 'completed':
      return 'success'
    case 'failed':
    case 'error':
      return 'failed'
    case 'cancelled':
      return 'cancelled'
    case 'timeout':
      return 'timeout'
    case 'pending_approval':
    case 'pending-approval':
      return 'pending-approval'
    case 'started':
    case 'running':
    case 'pending':
    default:
      return 'running'
  }
}

/** Convenience accessor: descriptor for a raw status string. */
export function toolStatusOf(raw: string | null | undefined, isError?: boolean): ToolStatusDescriptor {
  return TOOL_STATUS[toolStatusKey(raw, isError)]
}
