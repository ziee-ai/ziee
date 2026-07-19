import { Input, InputNumber, Segmented } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import {
  type BuilderStep,
  MAX_PARALLEL_HARD_CAP,
  configErrors,
} from './stepForms'
import { LabeledControl, PromptField } from './builderFields'
import { CapabilityMultiSelect } from './capabilities'

type LlmMapStep = Extract<BuilderStep, { kind: 'llm_map' }>

interface Props {
  store: WorkflowBuilderStore
  step: LlmMapStep
}

/** Run a prompt once per item in a list (fan-out). */
export function LlmMapStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  return (
    <div className="flex flex-col gap-4">
      <PromptField
        store={store}
        stepId={step.id}
        label="List to map over"
        value={step.for_each ?? ''}
        onChange={v => patch({ for_each: v })}
        placeholder="A reference to a list, e.g. {{ inputs.papers }} or a prior step's output"
        rows={2}
        required
        error={errors.for_each}
        testid="wf-builder-map-foreach"
      />

      <LabeledControl
        label="Item variable"
        description="The name each item is bound to inside the prompt, e.g. use {{ item }} in the prompt below."
        required
        error={errors.item_var}
      >
        <Input
          data-testid="wf-builder-map-itemvar"
          value={step.item_var ?? ''}
          onChange={e => patch({ item_var: e.target.value })}
          placeholder="item"
        />
      </LabeledControl>

      <PromptField
        store={store}
        stepId={step.id}
        label="Prompt (per item)"
        value={step.prompt ?? ''}
        onChange={v => patch({ prompt: v })}
        placeholder="Runs once per item. Reference the current item as {{ item }} (or your item-variable name)."
        rows={5}
        required
        error={errors.prompt}
        testid="wf-builder-map-prompt"
      />

      <LabeledControl
        label="Output"
        description="A written answer, or a structured (JSON) result per item."
      >
        <Segmented
          data-testid="wf-builder-map-output"
          aria-label="Output format"
          value={step.output_format === 'json' ? 'json' : 'text'}
          onValueChange={v => patch({ output_format: v })}
          options={[
            { value: 'text', label: 'Text' },
            { value: 'json', label: 'Structured' },
          ]}
        />
      </LabeledControl>

      <div className="flex flex-wrap gap-4">
        <LabeledControl
          label="Max in parallel"
          description={`How many items run at once (1–${MAX_PARALLEL_HARD_CAP}).`}
          error={errors.max_parallel}
        >
          <InputNumber
            data-testid="wf-builder-map-parallel"
            min={1}
            max={MAX_PARALLEL_HARD_CAP}
            precision={0}
            className="w-32"
            value={step.max_parallel ?? 5}
            onChange={v => patch({ max_parallel: v ?? 1 })}
          />
        </LabeledControl>

        <LabeledControl
          label="Retries per item"
          description="Retry a failed item this many times before applying the error policy."
          error={errors.max_retries}
        >
          <InputNumber
            data-testid="wf-builder-map-retries"
            min={0}
            precision={0}
            className="w-32"
            value={step.max_retries ?? 0}
            onChange={v => patch({ max_retries: v ?? 0 })}
          />
        </LabeledControl>
      </div>

      <LabeledControl
        label="On item error"
        description="Fail the whole step, skip the failing item, or retry it."
      >
        <Segmented
          data-testid="wf-builder-map-onerror"
          aria-label="On error"
          value={step.on_error ?? 'fail'}
          onValueChange={v => patch({ on_error: v })}
          options={[
            { value: 'fail', label: 'Fail' },
            { value: 'skip', label: 'Skip' },
            { value: 'retry', label: 'Retry' },
          ]}
        />
      </LabeledControl>

      <LabeledControl
        label="Tools"
        description="Optional tools the model may call for each item."
      >
        <CapabilityMultiSelect
          value={step.tools ?? []}
          onChange={v => patch({ tools: v })}
          testid="wf-builder-map-tools"
        />
      </LabeledControl>
    </div>
  )
}
