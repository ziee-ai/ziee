import { InputNumber } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, configErrors } from './stepForms'
import { LabeledControl, PromptField } from './builderFields'

type SandboxStep = Extract<BuilderStep, { kind: 'sandbox' }>

interface Props {
  store: WorkflowBuilderStore
  step: SandboxStep
}

/** Execute a shell command in the isolated code sandbox. */
export function SandboxStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  return (
    <div className="flex flex-col gap-4">
      <PromptField
        store={store}
        stepId={step.id}
        label="Command"
        value={step.run ?? ''}
        onChange={v => patch({ run: v })}
        placeholder="e.g. python analyse.py — references like {{ inputs.file }} are substituted before running"
        rows={4}
        required
        error={errors.run}
        testid="wf-builder-sandbox-run"
      />

      <PromptField
        store={store}
        stepId={step.id}
        label="Standard input"
        value={step.stdin ?? ''}
        onChange={v => patch({ stdin: v || null })}
        placeholder="Optional text piped to the command's stdin."
        rows={3}
        testid="wf-builder-sandbox-stdin"
      />

      <LabeledControl
        label="Timeout (ms)"
        description="Kill the command if it runs longer than this."
        error={errors.timeout_ms}
      >
        <InputNumber
          data-testid="wf-builder-sandbox-timeout"
          min={1}
          precision={0}
          className="w-40"
          value={step.timeout_ms ?? 30000}
          onChange={v => patch({ timeout_ms: v ?? 30000 })}
        />
      </LabeledControl>
    </div>
  )
}
