import { useEffect, useRef, useState } from 'react'
import { InputNumber, Textarea } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, configErrors } from './stepForms'
import { LabeledControl, PromptField } from './builderFields'

type ElicitStep = Extract<BuilderStep, { kind: 'elicit' }>

interface Props {
  store: WorkflowBuilderStore
  step: ElicitStep
}

const DEFAULT_SCHEMA = { type: 'object', properties: {} }

const schemaSnapshot = (s: unknown) => JSON.stringify(s ?? DEFAULT_SCHEMA)

/** Pause and collect structured input from a person. The `schema` is a JSON
 *  Schema describing the form to show. A raw-JSON editor is the sanctioned
 *  LAST-RESORT control here — a JSON-Schema blob has no simpler faithful UI and
 *  a full visual schema-builder is out of scope. This component is keyed by step
 *  id by the panel, so the local JSON buffer resets on step switch. */
export function ElicitStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  const [schemaText, setSchemaText] = useState(() =>
    JSON.stringify(step.schema ?? DEFAULT_SCHEMA, null, 2),
  )
  const [schemaError, setSchemaError] = useState<string | null>(null)

  // Snapshot of the schema as we last saw it in the store, so a cross-device
  // refetch that replaces `step.schema` resyncs the editor buffer, while our own
  // edits (already reflected in the store) don't clobber the text being typed.
  const lastPushed = useRef<string>(schemaSnapshot(step.schema))
  useEffect(() => {
    const incoming = schemaSnapshot(step.schema)
    if (incoming !== lastPushed.current) {
      lastPushed.current = incoming
      setSchemaText(JSON.stringify(step.schema ?? DEFAULT_SCHEMA, null, 2))
      setSchemaError(null)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step.schema])

  const onSchemaChange = (text: string) => {
    setSchemaText(text)
    try {
      const parsed = JSON.parse(text || '{}')
      setSchemaError(null)
      lastPushed.current = schemaSnapshot(parsed)
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
        {/* Raw JSON editor: the sanctioned last-resort control for a JSON-Schema
            blob (no simpler faithful UI; a visual schema-builder is out of
            scope). Needs an explicit accessible name of its own. */}
        <Textarea
          data-testid="wf-builder-elicit-schema"
          aria-label="Elicitation JSON schema"
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
