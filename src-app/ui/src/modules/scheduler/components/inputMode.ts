import type { Workflow } from '@/api-client/types'
import {
  type ParsedInput,
  parseWorkflowIr,
} from '@/modules/workflow/components/workflowIr'

/**
 * The declared inputs of the currently-selected workflow (ITEM-4). Pure so the
 * drawer's typed-vs-JSON decision is unit-testable (TEST-7). Returns [] when no
 * workflow is selected or the selected one declares no inputs.
 */
export function selectDeclaredInputs(
  workflows: Workflow[],
  workflowId: string,
): ParsedInput[] {
  const wf = workflows.find(w => w.id === workflowId)
  return wf ? parseWorkflowIr(wf).inputs : []
}

/**
 * Branch predicate: when the selected workflow declares ≥1 input the drawer
 * renders a typed field per input; otherwise it falls back to the free-form JSON
 * editor.
 */
export function chooseInputMode(inputs: ParsedInput[]): 'typed' | 'json' {
  return inputs.length > 0 ? 'typed' : 'json'
}
