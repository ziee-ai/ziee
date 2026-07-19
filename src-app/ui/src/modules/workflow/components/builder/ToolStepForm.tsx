import { useEffect, useRef, useState } from 'react'
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
  /** The editable text shown in the value input. */
  text: string
  /** The typed value this row commits. Authoritative (emitted UNCHANGED)
   *  while `text === baseText` — i.e. the row hasn't been edited since it was
   *  loaded or last re-derived. This is what preserves an untouched string
   *  arg's exact type (`"1234"` stays the string, not the number 1234). */
  value: unknown
  /** The `text` that `value` was derived from, captured on load. A row is
   *  "edited" (and its value re-derived from `text`) iff `text !== baseText`. */
  baseText: string
}

/** Does JSON-parsing `s` give the same string back? True when `s` isn't valid
 *  JSON at all (plain text, `{{ refs }}`) or parses to a string. When FALSE,
 *  `s` looks like a number/bool/null/object (`"1234"`, `"true"`, `{"a":1}`) and
 *  must be quoted so it survives a `parseValue` round-trip AS a string. */
function reparsesToString(s: string): boolean {
  try {
    return typeof JSON.parse(s.trim()) === 'string'
  } catch {
    return true
  }
}

/** Render a loaded argument value as round-trip-STABLE editor text: `parseValue`
 *  of the result reproduces the exact same typed value. A bare number/boolean/
 *  object/array and a `{{ ref }}` template render as-is; a STRING that would
 *  otherwise be reinterpreted (`"1234"`→number, `"true"`→bool) is JSON-quoted so
 *  it parses back to that same string. */
function toText(v: unknown): string {
  if (v === undefined) return ''
  if (typeof v === 'string') {
    // Bare string is fine only when `parseValue` would return it verbatim;
    // otherwise quote it (`"1234"`) so the round-trip preserves the string type.
    return reparsesToString(v) ? v : JSON.stringify(v)
  }
  // `null`→"null", number→"10", boolean→"true", object/array→JSON — each
  // re-parses back to the same typed value in `parseValue`.
  return JSON.stringify(v)
}

/** Turn a text field back into a typed value: parse it as JSON (so `10`→number,
 *  `true`→boolean, `null`, `[…]`, `{…}` round-trip as themselves); fall back to
 *  the raw string when it isn't valid JSON (covers plain text + `{{ refs }}`). */
function parseValue(raw: string): unknown {
  const trimmed = raw.trim()
  if (trimmed === '') return ''
  try {
    return JSON.parse(trimmed)
  } catch {
    return raw
  }
}

function argsToRows(args: unknown, nextRowId: () => number): ArgRow[] {
  if (!args || typeof args !== 'object' || Array.isArray(args)) return []
  return Object.entries(args as Record<string, unknown>).map(([key, value]) => {
    const text = toText(value)
    // Load-time value is authoritative: baseText === text, so an untouched row
    // re-emits `value` unchanged (never re-parsed) on subsequent commits.
    return { rowId: nextRowId(), key, text, value, baseText: text }
  })
}

/** Serialize rows to the arguments object. A row whose `text` is unchanged since
 *  load re-emits its EXACT loaded value (no re-parse — so editing row A can't
 *  coerce an untouched string row B); only a genuinely-edited row (`text !==
 *  baseText`) is re-derived from its text via `parseValue`. */
function rowsToArgs(rows: ArgRow[]): Record<string, unknown> {
  const obj: Record<string, unknown> = {}
  for (const r of rows) {
    if (r.key.trim()) {
      obj[r.key.trim()] = r.text === r.baseText ? r.value : parseValue(r.text)
    }
  }
  return obj
}

/** Call one specific tool on a server. Arguments use a key/value editor (each
 *  value may embed `{{ … }}` references, resolved at run time) rather than a raw
 *  JSON blob. Keyed by step id by the panel, so rows reset on step switch. */
export function ToolStepForm({ store, step }: Props) {
  const errors = configErrors(step)
  const patch = (p: Record<string, unknown>) => store.updateStep(step.id, p)

  // Stable monotonic row-id source (replaces the `useState(()=>({v}))[0]`
  // mutable-object anti-pattern). Rows are keyed by this id, never the index.
  const rowIdSeq = useRef(0)
  const nextRowId = () => {
    rowIdSeq.current += 1
    return rowIdSeq.current
  }

  const [rows, setRows] = useState<ArgRow[]>(() =>
    argsToRows(step.arguments, nextRowId),
  )

  // Serialized snapshot of the store's `arguments` as we last saw it, so we can
  // tell an external change (a sync refetch replacing `step.arguments`) apart
  // from our own commit and only resync the buffer for the former (FIX-F).
  const argsSnapshot = (a: unknown) =>
    JSON.stringify(a && typeof a === 'object' && !Array.isArray(a) ? a : {})
  const lastPushed = useRef<string>(argsSnapshot(step.arguments))

  useEffect(() => {
    const incoming = argsSnapshot(step.arguments)
    if (incoming !== lastPushed.current) {
      lastPushed.current = incoming
      setRows(argsToRows(step.arguments, nextRowId))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step.arguments])

  const commit = (next: ArgRow[]) => {
    setRows(next)
    const obj = rowsToArgs(next)
    lastPushed.current = JSON.stringify(obj)
    patch({ arguments: obj })
  }

  const addRow = () => {
    setRows([
      ...rows,
      { rowId: nextRowId(), key: '', text: '', value: '', baseText: '' },
    ])
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
                value={row.text}
                onChange={e =>
                  commit(
                    rows.map(r =>
                      r.rowId === row.rowId
                        ? { ...r, text: e.target.value }
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
