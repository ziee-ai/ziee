import { ChevronDown, ChevronUp, Plus, Trash2 } from 'lucide-react'
import { useState, useEffect, useRef } from 'react'
import React from 'react'
import {
  Button,
  Checkbox,
  FormField,
  FormList,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Text,
  useFormContext,
  useFormState,
  useWatch,
} from '@/components/ui'
import {
  type ArraySchema,
  type FieldSchema,
} from './workflowElicitSchema'

/** Below this row count the table renders all rows (no virtualization).
 *  At/above it (or when `ui.virtual` is set) we switch antd into virtual
 *  mode with a numeric `scroll.y` measured from the container — antd
 *  silently renders ALL rows if `scroll.y` is not a number. Copied from
 *  `file/viewers/tabular/DelimitedTable.tsx`. */
const VIRTUAL_ROW_THRESHOLD = 50

/** Rough height of the antd Table header row + internal padding; we
 *  subtract it from the measured container height so the scrollable body
 *  fills the remaining space (mirrors DelimitedTable). */
const TABLE_HEADER_PX = 48

/** Pixel width fallback for a column that doesn't declare `ui.width`. */
const DEFAULT_COL_WIDTH = 200

interface EditableArrayTableProps {
  /** Form path for this array property (the property name). */
  name: string
  /** The `type: 'array'` schema for the property. */
  schema: ArraySchema
  /** Array-level rules (required / minItems / maxItems) — no longer consumed
   *  directly; validation is handled by the parent form's zod resolver. Kept
   *  in the interface for backward-compat so callers that still pass it don't
   *  get a TS error. */
  listRules?: unknown[]
  disabled?: boolean
}

interface ColumnDef {
  key: string
  field: FieldSchema
  required: boolean
}

function columnDefs(schema: ArraySchema): ColumnDef[] {
  const itemProps = schema.items?.properties ?? {}
  const itemRequired = new Set(schema.items?.required ?? [])
  return Object.entries(itemProps).map(([key, field]) => ({
    key,
    field,
    required: itemRequired.has(key),
  }))
}

/** A single editable cell, rendered with the same widget logic as the
 *  scalar `renderField` (Input / InputNumber / Switch / Select-from-enum)
 *  driven by the column's `type`/`enum`. Per-cell zod rules from the parent
 *  form schema govern client-side validation parity. */
function EditableCell({
  namePrefix,
  col,
  disabled,
}: {
  /** Full dot-path prefix for this row, e.g. `"tableProp.2"`. */
  namePrefix: string
  col: ColumnDef
  disabled?: boolean
}) {
  const { field } = col
  const fullName = `${namePrefix}.${col.key}`

  if (field.enum) {
    return (
      <FormField name={fullName} aria-label={field.title || col.key} className="!mb-0">
        <Select
          data-testid={`wf-cell-select-${fullName}`}
          disabled={disabled}
          options={field.enum.map(v => ({ value: String(v), label: String(v) }))}
        />
      </FormField>
    )
  }
  if (field.type === 'boolean') {
    return (
      <FormField name={fullName} aria-label={field.title || col.key} valuePropName="checked" className="!mb-0">
        <Switch data-testid={`wf-cell-switch-${fullName}`} disabled={disabled} />
      </FormField>
    )
  }
  if (field.type === 'number' || field.type === 'integer') {
    return (
      <FormField name={fullName} aria-label={field.title || col.key} className="!mb-0">
        <InputNumber
          data-testid={`wf-cell-number-${fullName}`}
          min={field.minimum}
          max={field.maximum}
          precision={field.type === 'integer' ? 0 : undefined}
          disabled={disabled}
          className="w-full"
        />
      </FormField>
    )
  }
  return (
    <FormField name={fullName} aria-label={field.title || col.key} className="!mb-0">
      <Input data-testid={`wf-cell-input-${fullName}`} disabled={disabled} />
    </FormField>
  )
}

/** Reads a single cell value reactively for the expand-row display. */
function ExpandedCell({
  namePrefix,
  colKey,
}: {
  namePrefix: string
  colKey: string
}) {
  const value = useWatch({ name: `${namePrefix}.${colKey}` })
  return (
    <Text className="text-xs whitespace-pre-wrap">{String(value ?? '')}</Text>
  )
}

/**
 * Renders an editable table for a JSON-schema `type: 'array'`
 * property whose `items.type === 'object'` (or that carries an explicit
 * `ui.widget === 'table'`). It is rendered inside a parent `<Form>`; the
 * rows live in a `<FormList>` keyed on `name` so the edited rows become the
 * submitted value for that property.
 *
 * Optional `ui:` hint vocabulary (the schema stays valid without it):
 *   array-level  ui.{ widget:'table', virtual?, maxRows? }
 *   per-column   items.properties.<col>.ui.{ bulkToggle?, expand?,
 *                  sortable?, filterable?, width? }
 *
 * - `bulkToggle` on a boolean column  → row selection + a toolbar to set
 *   that column true/false on the selected rows.
 * - `expand` on a (long-text) column → an expandable row showing the full
 *   text of that cell.
 * - `sortable` / `filterable`        → NOTE: not implemented in the kit
 *   migration (the kit Table does not expose antd sorter/filter; schema
 *   hints are ignored and the columns render unsorted).
 */
export function EditableArrayTable({
  name,
  schema,
  listRules: _listRules, // consumed by parent (kept for compat), ignored here
  disabled,
}: EditableArrayTableProps) {
  const form = useFormContext<Record<string, unknown>>()
  const { errors } = useFormState()
  const arrayError = (errors as Record<string, { message?: string }>)[name]
    ?.message as string | undefined

  const cols = columnDefs(schema)
  const arrayUi = schema.ui ?? {}
  const bulkCol = cols.find(
    c => c.field.ui?.bulkToggle && c.field.type === 'boolean',
  )
  const expandCol = cols.find(c => c.field.ui?.expand)
  const maxRows = arrayUi.maxRows

  // Selection state for bulk operations (keyed by rhf `field.id`).
  const [selectedKeys, setSelectedKeys] = useState<string[]>([])

  // Expand state (keyed by rhf `field.id`).
  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set())

  // ResizeObserver-driven body height for virtual mode. We seed a sensible
  // non-zero default so the first paint is already virtualized; the
  // observer refines the exact height once layout settles.
  const wrapRef = useRef<HTMLDivElement>(null)
  const [bodyHeight, setBodyHeight] = useState<number>(360 - TABLE_HEADER_PX)
  useEffect(() => {
    if (!wrapRef.current) return
    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        const h = Math.floor(entry.contentRect.height) - TABLE_HEADER_PX
        if (h > 0) setBodyHeight(h)
      }
    })
    ro.observe(wrapRef.current)
    return () => ro.disconnect()
  }, [])

  const toggleExpand = (id: string) => {
    setExpandedRows(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <FormList name={name as any}>
      {({ fields, append, remove }) => {
        const useVirtual =
          arrayUi.virtual === true || fields.length >= VIRTUAL_ROW_THRESHOLD

        const atMax = maxRows !== undefined && fields.length >= maxRows
        const canRemoveBelowMin =
          schema.minItems === undefined || fields.length > schema.minItems

        // Bulk-set the bulkToggle column on the selected rows.
        const bulkSet = (value: boolean) => {
          if (!bulkCol) return
          const current =
            (form.getValues(name as any) as Array<Record<string, unknown>>) ??
            []
          const next = current.map((row, idx) => {
            const fieldForIdx = fields[idx]
            if (fieldForIdx && selectedKeys.includes(fieldForIdx.id)) {
              return { ...row, [bulkCol.key]: value }
            }
            return row
          })
          form.setValue(name as any, next as any)
        }

        const bulkDelete = () => {
          // Remove from the highest index down so earlier removals don't
          // shift the indices of rows still to be removed.
          const toRemove = fields
            .filter(f => selectedKeys.includes(f.id))
            .map((_, i) => i)
            .sort((a, b) => b - a)
          toRemove.forEach(i => remove(i))
          setSelectedKeys([])
        }

        const allSelected =
          fields.length > 0 &&
          fields.every(f => selectedKeys.includes(f.id))
        const someSelected = selectedKeys.length > 0 && !allSelected

        const colCount =
          cols.length +
          (bulkCol ? 1 : 0) +
          (expandCol ? 1 : 0) +
          1 // actions

        // Total column width for horizontal scroll.
        const totalWidth =
          (bulkCol ? 32 : 0) +
          cols.reduce((s, c) => s + (c.field.ui?.width ?? DEFAULT_COL_WIDTH), 0) +
          (expandCol ? 32 : 0) +
          48

        return (
          <div className="flex flex-col gap-2">
            {(bulkCol || selectedKeys.length > 0) && (
              <Space wrap>
                {bulkCol && (
                  <>
                    <Button
                      data-testid="wf-array-bulk-set-on-btn"
                      size="default"
                      disabled={disabled || selectedKeys.length === 0}
                      onClick={() => bulkSet(true)}
                    >
                      Set {bulkCol.field.title || bulkCol.key} on
                    </Button>
                    <Button
                      data-testid="wf-array-bulk-set-off-btn"
                      size="default"
                      disabled={disabled || selectedKeys.length === 0}
                      onClick={() => bulkSet(false)}
                    >
                      Set {bulkCol.field.title || bulkCol.key} off
                    </Button>
                  </>
                )}
                <Button variant="destructive"
                  data-testid="wf-array-bulk-delete-btn"
                  size="default"
                  disabled={disabled || selectedKeys.length === 0}
                  onClick={bulkDelete}
                >
                  Delete selected
                </Button>
                {selectedKeys.length > 0 && (
                  <Text className="text-xs text-muted-foreground">
                    {selectedKeys.length} selected
                  </Text>
                )}
              </Space>
            )}

            <div
              ref={wrapRef}
              className="overflow-x-auto"
              style={
                useVirtual
                  ? { height: bodyHeight + TABLE_HEADER_PX, overflowY: 'auto' }
                  : undefined
              }
            >
              <table
                style={{ minWidth: totalWidth }}
                className="w-full text-sm border-collapse"
              >
                <thead>
                  <tr className="border-b">
                    {bulkCol && (
                      <th className="w-8 px-1 py-1 text-left">
                        <Checkbox
                          data-testid="wf-array-select-all-checkbox"
                          checked={allSelected}
                          indeterminate={someSelected}
                          onChange={checked => {
                            setSelectedKeys(
                              checked ? fields.map(f => f.id) : [],
                            )
                          }}
                        />
                      </th>
                    )}
                    {cols.map(col => (
                      <th
                        key={col.key}
                        style={{
                          width: col.field.ui?.width ?? DEFAULT_COL_WIDTH,
                        }}
                        className="px-2 py-1 text-left font-medium text-muted-foreground"
                      >
                        {col.field.title || col.key}
                      </th>
                    ))}
                    {expandCol && <th className="w-8 px-1 py-1"></th>}
                    <th className="w-12 px-1 py-1"></th>
                  </tr>
                </thead>
                <tbody>
                  {fields.length === 0 ? (
                    <tr>
                      <td
                        colSpan={colCount}
                        className="py-4 text-center text-muted-foreground text-sm"
                      >
                        No rows
                      </td>
                    </tr>
                  ) : (
                    fields.map((field, i) => (
                      <React.Fragment key={field.id}>
                        <tr className="border-b hover:bg-muted/30">
                          {bulkCol && (
                            <td className="px-1 py-1">
                              <Checkbox
                                data-testid={`wf-array-row-checkbox-${field.id}`}
                                checked={selectedKeys.includes(field.id)}
                                onChange={checked => {
                                  setSelectedKeys(prev =>
                                    checked
                                      ? [...prev, field.id]
                                      : prev.filter(k => k !== field.id),
                                  )
                                }}
                              />
                            </td>
                          )}
                          {cols.map(col => (
                            <td key={col.key} className="px-1 py-1 align-top">
                              <EditableCell
                                namePrefix={`${name}.${i}`}
                                col={col}
                                disabled={disabled}
                              />
                            </td>
                          ))}
                          {expandCol && (
                            <td className="px-1 py-1 align-top">
                              <Button
                                data-testid={`wf-array-expand-btn-${field.id}`}
                                size="default"
                                type="button"
                                onClick={() => toggleExpand(field.id)}
                                aria-label={
                                  expandedRows.has(field.id)
                                    ? 'Collapse row'
                                    : 'Expand row'
                                }
                              >
                                {expandedRows.has(field.id) ? (
                                  <ChevronUp className="size-3.5" />
                                ) : (
                                  <ChevronDown className="size-3.5" />
                                )}
                              </Button>
                            </td>
                          )}
                          <td className="px-1 py-1 align-top">
                            <Button
                              data-testid={`wf-array-remove-btn-${field.id}`}
                              size="default"
                              type="button"
                              disabled={disabled || !canRemoveBelowMin}
                              aria-label="Remove row"
                              onClick={() => {
                                remove(i)
                                setSelectedKeys(prev =>
                                  prev.filter(k => k !== field.id),
                                )
                              }}
                            >
                              <Trash2 className="size-3.5" />
                            </Button>
                          </td>
                        </tr>
                        {expandCol && expandedRows.has(field.id) && (
                          <tr
                            key={`${field.id}-expand`}
                            className="border-b bg-muted/30"
                          >
                            <td
                              colSpan={colCount}
                              className="px-3 py-2"
                            >
                              <ExpandedCell
                                namePrefix={`${name}.${i}`}
                                colKey={expandCol.key}
                              />
                            </td>
                          </tr>
                        )}
                      </React.Fragment>
                    ))
                  )}
                </tbody>
              </table>
            </div>

            {arrayError && (
              <Text className="text-destructive text-sm">{arrayError}</Text>
            )}

            <Button
              data-testid="wf-array-add-row-btn"
              type="button"
              size="default"
              disabled={disabled || atMax}
              onClick={() => append(newRow(cols) as any)}
              className="w-full border-dashed"
            >
              <Plus className="size-3.5 mr-1" />
              Add row{atMax ? ` (max ${maxRows})` : ''}
            </Button>

            {/* Expose virtual-mode metrics via data attrs for tests / devtools */}
            {useVirtual && (
              <span
                aria-hidden
                data-virtual="true"
                data-body-height={bodyHeight}
                className="hidden"
              />
            )}
          </div>
        )
      }}
    </FormList>
  )
}

/** Build a blank row, seeding each column from its `default` (so a new row
 *  starts valid where possible). */
function newRow(cols: ColumnDef[]): Record<string, unknown> {
  const row: Record<string, unknown> = {}
  for (const col of cols) {
    row[col.key] =
      col.field.default ?? (col.field.type === 'boolean' ? false : undefined)
  }
  return row
}
