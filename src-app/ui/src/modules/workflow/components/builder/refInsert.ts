import type { InputDef } from '@/api-client/types'
import type { BuilderStep } from './stepForms'

// ---------------------------------------------------------------------------
// Pure reference enumeration for the RefInsertMenu (ITEM-10). Given the working
// definition and the index of the step being edited, produce the set of valid
// template references: every workflow input, plus every PRIOR step's output
// (a step can only reference steps that run before it). The token grammar
// matches the workflow template engine: `{{ inputs.<name> }}` for inputs and
// `{{ <step_id>.output }}` for a prior step's result (the step id is the head).
// ---------------------------------------------------------------------------

export interface RefOption {
  /** The exact token to insert into the field. */
  token: string
  /** Short human label (the input/step name). */
  label: string
  /** Grouping bucket for the menu. */
  group: 'Inputs' | 'Previous steps'
  /** Type / shape hint (e.g. `text`, `json`, `stdout`). */
  hint?: string
}

/** Best-effort output-shape hint per step kind, mirroring the IR type inference
 *  the backend `ref_check` performs (used here only as an author-facing hint). */
export function stepOutputHint(step: BuilderStep): string {
  switch (step.kind) {
    case 'llm':
    case 'agent':
      return step.output_format === 'json' ? 'json' : 'text'
    case 'llm_map':
      return step.output_format === 'json' ? 'json[]' : 'text[]'
    case 'sandbox':
      return 'stdout'
    case 'elicit':
      return 'form response'
    case 'tool':
      return 'tool result'
    default:
      return 'output'
  }
}

export function enumerateRefs(
  def: { inputs?: InputDef[]; steps?: BuilderStep[] },
  currentStepIndex: number,
): RefOption[] {
  const options: RefOption[] = []

  for (const input of def.inputs ?? []) {
    if (!input?.name) continue
    options.push({
      token: `{{ inputs.${input.name} }}`,
      label: input.name,
      group: 'Inputs',
      hint: input.required ? 'input · required' : 'input',
    })
  }

  const steps = def.steps ?? []
  const upperBound = currentStepIndex < 0 ? steps.length : currentStepIndex
  for (let i = 0; i < upperBound && i < steps.length; i += 1) {
    const step = steps[i]
    if (!step?.id) continue
    options.push({
      token: `{{ ${step.id}.output }}`,
      label: step.description?.trim() ? `${step.id} — ${step.description.trim()}` : step.id,
      group: 'Previous steps',
      hint: stepOutputHint(step),
    })
  }

  return options
}
