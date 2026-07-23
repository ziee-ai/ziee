// ---------------------------------------------------------------------------
// Pure helpers for the friendly agent-step form (ITEM-9). Kept out of the React
// component so the effort<->max_steps mapping and the plain-English read-back
// are unit-testable.
// ---------------------------------------------------------------------------

export const EFFORTS = ['quick', 'balanced', 'thorough'] as const
export type Effort = (typeof EFFORTS)[number]

/** Named effort levels → the agent's `max_steps` iteration ceiling. Discrete,
 *  friendlier stops than a raw number (there is no Slider in the kit). */
export const EFFORT_STEPS: Record<Effort, number> = {
  quick: 10,
  balanced: 30,
  thorough: 60,
}

export const EFFORT_LABELS: Record<Effort, string> = {
  quick: 'Quick',
  balanced: 'Balanced',
  thorough: 'Thorough',
}

export function effortToMaxSteps(effort: Effort): number {
  return EFFORT_STEPS[effort]
}

/** Nearest named effort for an arbitrary `max_steps` (a custom value entered in
 *  the Advanced disclosure highlights the closest stop on the Segmented). */
export function maxStepsToEffort(maxSteps: number): Effort {
  let best: Effort = 'balanced'
  let bestDiff = Number.POSITIVE_INFINITY
  for (const effort of EFFORTS) {
    const diff = Math.abs(EFFORT_STEPS[effort] - maxSteps)
    if (diff < bestDiff) {
      bestDiff = diff
      best = effort
    }
  }
  return best
}

/** Whether `max_steps` sits exactly on a named stop (else the value is custom). */
export function isCustomMaxSteps(maxSteps: number): boolean {
  return !EFFORTS.some(e => EFFORT_STEPS[e] === maxSteps)
}

export interface AgentReadbackConfig {
  prompt?: string | null
  max_steps?: number
  output_format?: 'text' | 'json'
  /** Human labels of the selected capabilities (server display names). */
  capabilityLabels: string[]
}

/** A show-then-act sentence describing what the configured agent task will do,
 *  in plain language a non-technical author can sanity-check. */
export function agentReadback(config: AgentReadbackConfig): string {
  const task = (config.prompt ?? '').trim()
  const firstLine = task.split('\n')[0]?.trim() ?? ''
  const goal = firstLine
    ? `try to "${truncate(firstLine, 120)}"`
    : 'run the task you describe above'

  const caps = config.capabilityLabels.filter(Boolean)
  const tools =
    caps.length === 0
      ? 'without any tools'
      : caps.length <= 3
        ? `using ${joinList(caps)}`
        : `using ${caps.length} capabilities`

  const maxSteps = config.max_steps ?? EFFORT_STEPS.balanced
  const effort = `taking up to ${maxSteps} step${maxSteps === 1 ? '' : 's'}`

  const output =
    config.output_format === 'json'
      ? 'and return a structured result'
      : 'and return a written answer'

  return `The assistant will ${goal}, ${tools}, ${effort}, ${output}.`
}

function truncate(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, max - 1)}…` : s
}

function joinList(items: string[]): string {
  if (items.length === 1) return items[0]
  if (items.length === 2) return `${items[0]} and ${items[1]}`
  return `${items.slice(0, -1).join(', ')}, and ${items[items.length - 1]}`
}
