import { useState } from 'react'
import { InputNumber, Textarea } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, configErrors } from './stepForms'
import { LabeledControl, PromptField } from './builderFields'

type ElicitStep = Extract<BuilderStep, { kind: 'elicit' }>

interface Props {
  store: WorkflowBuilderStore
  step: ElicitStep
}

/** Pause and collect structured input from a person. The `schema` is a JSON
 *  Schema describing the form to show — the one place a JSON editor is the
 *  pragmatic control (a full schema-builder is out of scope). This component is
 *  keyed by step id by the panel, so the local JSON buffer resets on step switch. */
export function ElicitStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  const [schemaText, setSchemaText] = useState(() =>
    JSON.stringify(step.schema ?? { type: 'object', properties: {} }, null, 2),
  )
  const [schemaError, setSchemaError] = useState<string | null>(null)

  const onSchemaChange = (text: string) => {
    setSchemaText(text)
    try {
      const parsed = JSON.parse(text || '{}')
      setSchemaError(null)
      patch({ schema: parsed })
    } catch {
      setSchemaError('Schema must be valid JSON')
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PromptField
        store={store}
        stepId={step.id}
        label="Prompt for the user"
        value={step.message ?? ''}
        onChange={v => patch({ message: v })}
        placeholder="What are you asking the person to provide? e.g. “Review the screened papers and confirm which to include.”"
        rows={3}
        required
        error={errors.message}
        testid="wf-builder-elicit-message"
      />

      <LabeledControl
        label="Form schema (JSON Schema)"
        description="Describes the fields shown to the user. An object schema with a `properties` map."
        error={schemaError}
      >
        <Textarea
          data-testid="wf-builder-elicit-schema"
          rows={8}
          value={schemaText}
          onChange={e => onSchemaChange(e.target.value)}
          className="font-mono text-xs"
        />
      </LabeledControl>

      <LabeledControl
        label="Wait timeout (ms)"
        description="How long to wait for a response. Use 0 to wait indefinitely (a durable human gate)."
        error={errors.timeout_ms}
      >
        <InputNumber
          data-testid="wf-builder-elicit-timeout"
          min={0}
          precision={0}
          className="w-40"
          value={step.timeout_ms ?? 300000}
          onChange={v => patch({ timeout_ms: v ?? 0 })}
        />
      </LabeledControl>
    </div>
  )
}
