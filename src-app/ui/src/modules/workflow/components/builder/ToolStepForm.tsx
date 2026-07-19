import { useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { Button, Input } from '@ziee/kit'
import type { WorkflowBuilderStore } from '../../stores/WorkflowBuilder.store'
import { type BuilderStep, configErrors } from './stepForms'
import { LabeledControl } from './builderFields'
import { CapabilitySelect } from './capabilities'

type ToolStep = Extract<BuilderStep, { kind: 'tool' }>

interface Props {
  store: WorkflowBuilderStore
  step: ToolStep
}

interface ArgRow {
  rowId: number
  key: string
  value: string
}

/** Stringify a loaded argument value for the key/value editor (objects/arrays
 *  are shown as JSON; primitives as their text). */
function toText(v: unknown): string {
  if (v == null) return ''
  if (typeof v === 'string') return v
  return JSON.stringify(v)
}

function argsToRows(args: unknown): ArgRow[] {
  if (!args || typeof args !== 'object' || Array.isArray(args)) return []
  return Object.entries(args as Record<string, unknown>).map(([key, value], i) => ({
    rowId: i,
    key,
    value: toText(value),
  }))
}

/** Call one specific tool on a server. Arguments use a key/value editor (each
 *  value may embed `{{ … }}` references, resolved at run time) rather than a raw
 *  JSON blob. Keyed by step id by the panel, so rows reset on step switch. */
export function ToolStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  const [rows, setRows] = useState<ArgRow[]>(() => argsToRows(step.arguments))
  const nextId = useState(() => ({ v: rows.length }))[0]

  const commit = (next: ArgRow[]) => {
    setRows(next)
    const obj: Record<string, string> = {}
    for (const r of next) {
      if (r.key.trim()) obj[r.key.trim()] = r.value
    }
    patch({ arguments: obj })
  }

  const addRow = () => {
    nextId.v += 1
    setRows([...rows, { rowId: nextId.v, key: '', value: '' }])
  }

  return (
    <div className="flex flex-col gap-4">
      <LabeledControl label="Server" required error={errors.server}>
        <CapabilitySelect
          value={step.server ?? ''}
          onChange={v => patch({ server: v })}
          testid="wf-builder-tool-server"
        />
      </LabeledControl>

      <LabeledControl
        label="Tool"
        description="The exact name of the tool to call on that server."
        required
        error={errors.tool}
      >
        <Input
          data-testid="wf-builder-tool-name"
          value={step.tool ?? ''}
          onChange={e => patch({ tool: e.target.value })}
          placeholder="e.g. search"
        />
      </LabeledControl>

      <LabeledControl
        label="Arguments"
        description="Key/value pairs passed to the tool. A value may reference an input or prior step, e.g. {{ inputs.query }}."
      >
        <div className="flex flex-col gap-2">
          {rows.length === 0 && (
            <span className="text-xs text-muted-foreground">No arguments</span>
          )}
          {rows.map((row, i) => (
            <div key={row.rowId} className="flex items-center gap-2">
              <Input
                data-testid={`wf-builder-tool-arg-key-${i}`}
                aria-label="Argument name"
                className="w-1/3"
                value={row.key}
                onChange={e =>
                  commit(
                    rows.map(r =>
                      r.rowId === row.rowId ? { ...r, key: e.target.value } : r,
                    ),
                  )
                }
                placeholder="name"
              />
              <Input
                data-testid={`wf-builder-tool-arg-value-${i}`}
                aria-label="Argument value"
                className="flex-1"
                value={row.value}
                onChange={e =>
                  commit(
                    rows.map(r =>
                      r.rowId === row.rowId
                        ? { ...r, value: e.target.value }
                        : r,
                    ),
                  )
                }
                placeholder="value or {{ reference }}"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                icon={<Trash2 />}
                aria-label="Remove argument"
                data-testid={`wf-builder-tool-arg-remove-${i}`}
                onClick={() => commit(rows.filter(r => r.rowId !== row.rowId))}
              />
            </div>
          ))}
          <Button
            type="button"
            variant="outline"
            icon={<Plus />}
            data-testid="wf-builder-tool-arg-add"
            onClick={addRow}
            className="self-start"
          >
            Add argument
          </Button>
        </div>
      </LabeledControl>
    </div>
  )
}
