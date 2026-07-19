import { z } from 'zod'
import type { StepDef } from '@/api-client/types'

// ---------------------------------------------------------------------------
// Pure, unit-testable module backing the workflow builder's per-kind step forms.
//
// The generated `StepDef` is a FLAT tagged union discriminated by `kind` (the
// backend `#[serde(flatten)]`s `StepConfig` onto `StepDef`, so the wire shape
// carries `kind` + the config fields directly on the step). The generator is
// FLATTEN-LOSSY: it drops the shared base fields (`id`, `description`,
// `message`, `depends_on`, …) from the TS type even though serde flatten still
// emits + accepts them on the wire. We re-add them here as `StepBase` and
// intersect: an intersection with a union distributes, giving a discriminated
// union of steps that each carry the base fields AND their kind's config.
//
// Drift-safety — why there is NO compile-time guard here:
//  A type-level `BuilderStep extends StepDef` assertion is a TAUTOLOGY and
//  guards nothing: `BuilderStep = StepBase & StepDef` and the wire step element
//  resolves to the same imported `StepDef`, so `(StepBase & StepDef) extends
//  StepDef` is unconditionally true. A genuine compile-time drift check is
//  impossible while the generated `StepDef` is itself flatten-lossy — both the
//  builder and wire sides derive from the same lossy type, so neither can catch
//  a base-field change the codegen already dropped. The runtime WIRE is
//  nevertheless correct because serde-flatten emits + accepts every field
//  (base + config). Drift is caught by the backend def→bundle round-trip
//  INTEGRATION test, not a compile-time check; the real fix — a non-lossy
//  `emit_ts` generator that keeps the flattened base fields on `StepDef` — is
//  the tracked follow-up.
// ---------------------------------------------------------------------------

export const STEP_KINDS = [
  'agent',
  'llm',
  'llm_map',
  'sandbox',
  'elicit',
  'tool',
] as const

export type StepKind = (typeof STEP_KINDS)[number]

/** Shared step fields the generated `StepDef` union omits (see module note). */
export interface StepBase {
  id: string
  description?: string | null
  /** The elicit prompt (kind `elicit`) OR a dynamic status line for others. */
  message?: string | null
  depends_on?: string[]
}

export type BuilderStep = StepBase & StepDef

/** Domain-language label per kind. The agent kind is deliberately named in
 *  plain terms ("AI assistant task") rather than tool jargon. */
export const STEP_KIND_LABELS: Record<StepKind, string> = {
  agent: 'AI assistant task',
  llm: 'LLM prompt',
  llm_map: 'Map over a list',
  sandbox: 'Run code',
  elicit: 'Ask the user',
  tool: 'Call a tool',
}

/** One-line helper text shown in the add-step menu. */
export const STEP_KIND_DESCRIPTIONS: Record<StepKind, string> = {
  agent: 'An autonomous assistant that uses tools to complete a task',
  llm: 'A single prompt to the language model',
  llm_map: 'Run a prompt once per item in a list',
  sandbox: 'Execute a shell command in an isolated sandbox',
  elicit: 'Pause and collect input from a person',
  tool: 'Invoke one specific tool on a server',
}

/** Default agent iteration ceiling (mirrors backend `default_agent_max_steps`). */
export const DEFAULT_AGENT_MAX_STEPS = 30
export const DEFAULT_MAX_PARALLEL = 5
export const MAX_PARALLEL_HARD_CAP = 20
export const DEFAULT_SANDBOX_TIMEOUT_MS = 30_000
export const DEFAULT_ELICIT_TIMEOUT_MS = 300_000

/** Build a fresh step id `<kind>_<n>` unique against `existingIds`. Pure so it
 *  can be unit-tested independent of the store. */
export function nextStepId(kind: StepKind, existingIds: string[]): string {
  const taken = new Set(existingIds)
  let n = 1
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const candidate = `${kind}_${n}`
    if (!taken.has(candidate)) return candidate
    n += 1
  }
}

/** A brand-new step of the given kind with sensible defaults. */
export function createStep(kind: StepKind, existingIds: string[]): BuilderStep {
  const id = nextStepId(kind, existingIds)
  const base: StepBase = { id, description: '', depends_on: [] }
  switch (kind) {
    case 'agent':
      return {
        ...base,
        kind: 'agent',
        prompt: '',
        system: null,
        servers: [],
        max_steps: DEFAULT_AGENT_MAX_STEPS,
        output_format: 'text',
      }
    case 'llm':
      // No `tools`: the backend rejects a non-empty `tools` on an llm step
      // (validate.rs E6, WORKFLOW_DEAD_TOOLS_FIELD). Omitted here so the field
      // stays absent from the wire payload.
      return {
        ...base,
        kind: 'llm',
        prompt: '',
        output_format: 'text',
      }
    case 'llm_map':
      // No `tools` — same reason as the `llm` kind above.
      return {
        ...base,
        kind: 'llm_map',
        prompt: '',
        for_each: '',
        item_var: 'item',
        output_format: 'text',
        max_parallel: DEFAULT_MAX_PARALLEL,
        max_retries: 0,
        on_error: 'fail',
      }
    case 'sandbox':
      return {
        ...base,
        kind: 'sandbox',
        run: '',
        stdin: null,
        timeout_ms: DEFAULT_SANDBOX_TIMEOUT_MS,
      }
    case 'elicit':
      return {
        ...base,
        kind: 'elicit',
        message: '',
        schema: { type: 'object', properties: {} },
        timeout_ms: DEFAULT_ELICIT_TIMEOUT_MS,
      }
    case 'tool':
      return {
        ...base,
        kind: 'tool',
        server: '',
        tool: '',
        arguments: {},
      }
  }
}

// ---------------------------------------------------------------------------
// Per-kind zod schema — validates a step's config for inline field feedback in
// the forms. The backend `POST /validate-def` remains the source of truth (its
// findings render in the validation panel); these client schemas only surface
// required-field hints as the author types. Pure + exported for unit tests.
// ---------------------------------------------------------------------------

const nonEmpty = (label: string) => z.string().trim().min(1, `${label} is required`)

export function buildStepZodSchema(kind: StepKind): z.ZodTypeAny {
  switch (kind) {
    case 'agent':
      return z.object({
        prompt: nonEmpty('A task description'),
        max_steps: z
          .number({ message: 'Enter a number' })
          .int()
          .min(1, 'Must be at least 1'),
        output_format: z.enum(['text', 'json']),
      })
    case 'llm':
      return z.object({
        prompt: nonEmpty('A prompt'),
        output_format: z.enum(['text', 'json']),
      })
    case 'llm_map':
      return z.object({
        prompt: nonEmpty('A prompt'),
        for_each: nonEmpty('A list to map over'),
        item_var: nonEmpty('An item variable name'),
        max_parallel: z
          .number({ message: 'Enter a number' })
          .int()
          .min(1, 'Must be at least 1')
          .max(MAX_PARALLEL_HARD_CAP, `At most ${MAX_PARALLEL_HARD_CAP}`),
        max_retries: z.number().int().min(0, 'Cannot be negative'),
      })
    case 'sandbox':
      return z.object({
        run: nonEmpty('A command to run'),
        timeout_ms: z.number().int().min(1, 'Must be at least 1 ms'),
      })
    case 'elicit':
      return z.object({
        message: nonEmpty('A prompt for the user'),
        timeout_ms: z.number().int().min(0, 'Cannot be negative'),
      })
    case 'tool':
      return z.object({
        server: nonEmpty('A server'),
        tool: nonEmpty('A tool name'),
      })
  }
}

/** Run the kind schema against a step and return `{ fieldName: message }` for
 *  the fields that fail. Never throws (unknown-shape input → best-effort). */
export function configErrors(step: BuilderStep): Record<string, string> {
  const schema = buildStepZodSchema(step.kind as StepKind)
  const result = schema.safeParse(step)
  if (result.success) return {}
  const errors: Record<string, string> = {}
  for (const issue of result.error.issues) {
    const key = String(issue.path[0] ?? '')
    if (key && !(key in errors)) errors[key] = issue.message
  }
  return errors
}
