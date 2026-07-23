import { Segmented } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, configErrors } from './stepForms'
import { LabeledControl, PromptField } from './builderFields'

type LlmStep = Extract<BuilderStep, { kind: 'llm' }>

interface Props {
  store: WorkflowBuilderStore
  step: LlmStep
}

/** A single language-model prompt. */
export function LlmStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  return (
    <div className="flex flex-col gap-4">
      <PromptField
        store={store}
        stepId={step.id}
        label="Prompt"
        value={step.prompt ?? ''}
        onChange={v => patch({ prompt: v })}
        placeholder="Write the prompt. Insert a reference to reuse an input or a prior step's output."
        rows={6}
        required
        error={errors.prompt}
        testid="wf-builder-llm-prompt"
      />

      <LabeledControl
        label="Output"
        description="A written answer, or a structured (JSON) result."
      >
        <Segmented
          data-testid="wf-builder-llm-output"
          aria-label="Output format"
          value={step.output_format === 'json' ? 'json' : 'text'}
          onValueChange={v => patch({ output_format: v })}
          options={[
            { value: 'text', label: 'Text' },
            { value: 'json', label: 'Structured' },
          ]}
        />
      </LabeledControl>

      {/* No tools picker here: the backend `validate-def` (E6,
          WORKFLOW_DEAD_TOOLS_FIELD) rejects a non-empty `tools` on an
          llm/llm_map step. Tool use belongs to the `tool` step kind (a single
          named tool) and to the agent step's `servers` (capabilities). */}
    </div>
  )
}
