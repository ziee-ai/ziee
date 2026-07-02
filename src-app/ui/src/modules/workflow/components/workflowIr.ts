import type { Workflow } from '@/api-client/types'

/** Defensively-parsed input declaration from a workflow's compiled IR.
 *  The full definition lives in `workflow.yaml` on disk; the server may
 *  expose a compiled form via `compiled_ir_json`. When absent the run
 *  dialog falls back to a free-form JSON inputs editor. */
export interface ParsedInput {
  name: string
  description?: string
  required: boolean
  default?: unknown
}

export interface ParsedStep {
  id: string
  kind?: string
  dependsOn?: string[]
  /** Author-facing step label. The compiled IR carries it as `description`
   *  (IrStep.description); `message` is the elicit-prompt/status line, not the
   *  static label, so the drawer shows `description`. */
  description?: string
}

export interface ParsedWorkflowIr {
  inputs: ParsedInput[]
  steps: ParsedStep[]
}

/** Best-effort extraction of inputs[] + steps[] from a workflow's
 *  `compiled_ir_json`. Returns empty arrays when the IR isn't present
 *  or has an unexpected shape. */
export function parseWorkflowIr(workflow: Workflow): ParsedWorkflowIr {
  const ir = workflow.compiled_ir_json
  if (!ir || typeof ir !== 'object') {
    return { inputs: [], steps: [] }
  }
  const raw = ir as Record<string, unknown>

  const inputs: ParsedInput[] = Array.isArray(raw.inputs)
    ? (raw.inputs as Record<string, unknown>[])
        .filter(i => i && typeof i.name === 'string')
        .map(i => ({
          name: i.name as string,
          description:
            typeof i.description === 'string' ? i.description : undefined,
          required: i.required === true,
          default: i.default,
        }))
    : []

  const steps: ParsedStep[] = Array.isArray(raw.steps)
    ? (raw.steps as Record<string, unknown>[])
        .filter(s => s && typeof s.id === 'string')
        .map(s => ({
          id: s.id as string,
          kind: typeof s.kind === 'string' ? s.kind : undefined,
          dependsOn: Array.isArray(s.depends_on)
            ? (s.depends_on as string[])
            : undefined,
          description:
            typeof s.description === 'string' ? s.description : undefined,
        }))
    : []

  return { inputs, steps }
}
