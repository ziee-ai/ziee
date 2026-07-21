import { Empty, Input, Tag, Text } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, STEP_KIND_LABELS, type StepKind } from './stepForms'
import { LabeledControl } from './builderFields'
import { AgentStepForm } from './AgentStepForm'
import { LlmStepForm } from './LlmStepForm'
import { LlmMapStepForm } from './LlmMapStepForm'
import { SandboxStepForm } from './SandboxStepForm'
import { ElicitStepForm } from './ElicitStepForm'
import { ToolStepForm } from './ToolStepForm'

interface StepConfigPanelProps {
  store: WorkflowBuilderStore
}

/** Dispatch to the per-kind form by `step.kind`. Each form is keyed by step id
 *  so its local buffers (elicit JSON schema, tool arg rows) reset on switch. */
function StepForm({
  store,
  step,
}: {
  store: WorkflowBuilderStore
  step: BuilderStep
}) {
  switch (step.kind) {
    case 'agent':
      return <AgentStepForm store={store} step={step} />
    case 'llm':
      return <LlmStepForm store={store} step={step} />
    case 'llm_map':
      return <LlmMapStepForm store={store} step={step} />
    case 'sandbox':
      return <SandboxStepForm store={store} step={step} />
    case 'elicit':
      return <ElicitStepForm store={store} step={step} />
    case 'tool':
      return <ToolStepForm store={store} step={step} />
    default:
      return null
  }
}

/** ITEM-7 — the detail column: shared step-level fields + the kind-specific form. */
export function StepConfigPanel({ store }: StepConfigPanelProps) {
  const steps = store.def.steps
  const selectedStepId = store.selectedStepId
  const step = steps.find(s => s.id === selectedStepId)

  if (!step) {
    return (
      <Empty
        data-testid="wf-builder-no-step-selected"
        description="Select a step to configure it"
      />
    )
  }

  return (
    <div className="flex flex-col gap-4" data-testid="wf-builder-step-config">
      <div className="flex items-center gap-2 flex-wrap">
        <Text strong className="truncate">
          {step.description?.trim() || step.id}
        </Text>
        <Tag
          variant="outline"
          tone="info"
          className="text-xs"
          data-testid="wf-builder-step-config-kind"
        >
          {STEP_KIND_LABELS[step.kind as StepKind] ?? step.kind}
        </Tag>
      </div>

      <LabeledControl
        label="Step label"
        description="A short, human-readable name shown in the run timeline (optional)."
      >
        <Input
          data-testid="wf-builder-step-description"
          value={step.description ?? ''}
          onChange={e =>
            store.updateStep(step.id, { description: e.target.value })
          }
          placeholder={step.id}
        />
      </LabeledControl>

      <StepForm key={step.id} store={store} step={step} />
    </div>
  )
}
