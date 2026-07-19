import { Plus, Trash2 } from 'lucide-react'
import { Button, Empty, Input, Switch, Text } from '@ziee/kit'
import type { InputDef } from '@/api-client/types'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { LabeledControl } from './builderFields'

interface WorkflowInputsEditorProps {
  store: WorkflowBuilderStore
}

/** ITEM-7 — edit the workflow's declared inputs (name / description / required).
 *  Inputs become `{{ inputs.<name> }}` references available to every step. */
export function WorkflowInputsEditor({ store }: WorkflowInputsEditorProps) {
  const inputs = store.def.inputs

  const update = (next: InputDef[]) => store.updateInputs(next)
  const patchAt = (i: number, patch: Partial<InputDef>) =>
    update(inputs.map((input, idx) => (idx === i ? { ...input, ...patch } : input)))

  return (
    <div className="flex flex-col gap-3" data-testid="wf-builder-inputs-editor">
      <div className="flex items-center justify-between gap-2">
        <Text strong>Inputs</Text>
        <Button
          type="button"
          variant="outline"
          icon={<Plus />}
          data-testid="wf-builder-input-add"
          onClick={() =>
            update([...inputs, { name: '', description: '', required: false }])
          }
        >
          Add input
        </Button>
      </div>

      {inputs.length === 0 ? (
        <Empty
          data-testid="wf-builder-inputs-empty"
          description="No inputs — add one if the workflow needs values from the person who runs it"
        />
      ) : (
        <div className="flex flex-col gap-3">
          {inputs.map((input, i) => (
            <div
              key={i}
              className="flex flex-col gap-2 rounded-md border border-border p-3"
              data-testid={`wf-builder-input-row-${i}`}
            >
              <div className="flex items-start gap-2">
                <div className="flex-1">
                  <LabeledControl label="Name" required>
                    <Input
                      data-testid={`wf-builder-input-name-${i}`}
                      value={input.name}
                      onChange={e => patchAt(i, { name: e.target.value })}
                      placeholder="e.g. topic"
                    />
                  </LabeledControl>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  icon={<Trash2 />}
                  aria-label="Remove input"
                  data-testid={`wf-builder-input-remove-${i}`}
                  className="mt-6"
                  onClick={() => update(inputs.filter((_, idx) => idx !== i))}
                />
              </div>
              <LabeledControl label="Description">
                <Input
                  data-testid={`wf-builder-input-desc-${i}`}
                  value={input.description ?? ''}
                  onChange={e => patchAt(i, { description: e.target.value })}
                  placeholder="What is this value for?"
                />
              </LabeledControl>
              <div className="flex items-center gap-2">
                <Switch
                  data-testid={`wf-builder-input-required-${i}`}
                  checked={input.required ?? false}
                  onChange={v => patchAt(i, { required: v })}
                  size="sm"
                />
                <Text type="secondary" className="text-xs">
                  Required
                </Text>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
