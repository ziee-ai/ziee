import { useRef } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { Button, Empty, Input, SectionHeader, Switch, Text } from '@ziee/kit'
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

  // `InputDef` carries no id, so keep a client-side stable id per row (React key)
  // rather than the array index. Add/remove keep this list in lockstep with
  // `inputs`; the tail back-fill covers initial load / a cross-device replace.
  const idSeq = useRef(0)
  const rowIds = useRef<string[]>([])
  while (rowIds.current.length < inputs.length) {
    idSeq.current += 1
    rowIds.current.push(`wf-input-${idSeq.current}`)
  }
  if (rowIds.current.length > inputs.length) {
    rowIds.current.length = inputs.length
  }

  const update = (next: InputDef[]) => store.updateInputs(next)
  const patchAt = (i: number, patch: Partial<InputDef>) =>
    update(inputs.map((input, idx) => (idx === i ? { ...input, ...patch } : input)))
  const addRow = () => {
    idSeq.current += 1
    rowIds.current.push(`wf-input-${idSeq.current}`)
    update([...inputs, { name: '', description: '', required: false }])
  }
  const removeAt = (i: number) => {
    rowIds.current.splice(i, 1)
    update(inputs.filter((_, idx) => idx !== i))
  }

  return (
    <div className="flex flex-col gap-3" data-testid="wf-builder-inputs-editor">
      <SectionHeader
        title="Inputs"
        data-testid="wf-builder-inputs-header"
        actions={
          <Button
            type="button"
            variant="outline"
            icon={<Plus />}
            data-testid="wf-builder-input-add"
            onClick={addRow}
          >
            Add input
          </Button>
        }
      />

      {inputs.length === 0 ? (
        <Empty
          data-testid="wf-builder-inputs-empty"
          description="No inputs — add one if the workflow needs values from the person who runs it"
        />
      ) : (
        <div className="flex flex-col gap-3">
          {inputs.map((input, i) => (
            <div
              key={rowIds.current[i]}
              className="flex flex-col gap-2 rounded-md border border-border p-3"
              data-testid={`wf-builder-input-row-${i}`}
            >
              {/* items-end aligns the remove button with the BOTTOM of the Name
                  field (its input), so it stays aligned even when the label
                  wraps to two lines on a narrow (≤390px) viewport — no `mt-6`
                  magic offset that assumes a single-line label. */}
              <div className="flex items-end gap-2">
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
                  onClick={() => removeAt(i)}
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
                  aria-label={`Mark input ${input.name || i + 1} as required`}
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
