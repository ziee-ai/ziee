import {
  Trash2,
  Plus,
} from 'lucide-react'
import {
  Button,
  Form,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Table,
  Typography,
} from 'antd'
import type { TableColumnsType } from 'antd'
import type { FormInstance, Rule } from 'antd/es/form'
import { useEffect, useRef, useState } from 'react'
import {
  type ArraySchema,
  type FieldSchema,
  type ListRule,
  fieldItemRules,
} from './workflowElicitSchema'

const { Text } = Typography

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
  /** The form instance, used to read selected-row values for bulk ops. */
  form: FormInstance
  /** Array-level rules (required / minItems / maxItems) applied to the
   *  `Form.List` so an invalid list blocks submit. */
  listRules?: ListRule[]
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
 *  driven by the column's `type`/`enum`. Per-cell schema rules are applied
 *  for client-side validation parity. */
function EditableCell({
  rowName,
  col,
  disabled,
}: {
  rowName: number
  col: ColumnDef
  disabled?: boolean
}) {
  const { field } = col
  const rules: Rule[] = fieldItemRules(field, col.required)

  if (field.enum) {
    return (
      <Form.Item name={[rowName, col.key]} rules={rules} className="!mb-0">
        <Select
          size="small"
          allowClear={!col.required}
          disabled={disabled}
          options={field.enum.map(v => ({ value: v, label: String(v) }))}
        />
      </Form.Item>
    )
  }
  if (field.type === 'boolean') {
    return (
      <Form.Item
        name={[rowName, col.key]}
        valuePropName="checked"
        className="!mb-0"
      >
        <Switch size="small" disabled={disabled} />
      </Form.Item>
    )
  }
  if (field.type === 'number' || field.type === 'integer') {
    return (
      <Form.Item name={[rowName, col.key]} rules={rules} className="!mb-0">
        <InputNumber
          size="small"
          min={field.minimum}
          max={field.maximum}
          precision={field.type === 'integer' ? 0 : undefined}
          disabled={disabled}
          style={{ width: '100%' }}
        />
      </Form.Item>
    )
  }
  return (
    <Form.Item name={[rowName, col.key]} rules={rules} className="!mb-0">
      <Input size="small" disabled={disabled} />
    </Form.Item>
  )
}

/**
 * Renders an editable antd `Table` for a JSON-schema `type: 'array'`
 * property whose `items.type === 'object'` (or that carries an explicit
 * `ui.widget === 'table'`). It is wrapped by the caller in a
 * `<Form.Item name={prop}>`, but the rows live in a `Form.List` keyed on
 * `prop` so the edited rows become the submitted value for that property.
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
 * - `sortable` / `filterable`        → antd column `sorter` / `filters`.
 */
export function EditableArrayTable({
  name,
  schema,
  form,
  listRules,
  disabled,
}: EditableArrayTableProps) {
  const cols = columnDefs(schema)
  const arrayUi = schema.ui ?? {}
  const bulkCol = cols.find(c => c.field.ui?.bulkToggle && c.field.type === 'boolean')
  const expandCol = cols.find(c => c.field.ui?.expand)
  const maxRows = arrayUi.maxRows

  // Selection state for bulk operations (keyed by Form.List field key).
  const [selectedKeys, setSelectedKeys] = useState<React.Key[]>([])

  // ResizeObserver-driven body height for virtual mode. We seed a sensible
  // non-zero default so the first paint is already virtualized; the
  // observer refines the exact height once layout settles.
  const wrapRef = useRef<HTMLDivElement>(null)
  const [bodyHeight, setBodyHeight] = useState<number>(360)
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

  return (
    <Form.List name={name} rules={listRules}>
      {(fields, { add, remove }, { errors }) => {
        const useVirtual =
          arrayUi.virtual === true || fields.length >= VIRTUAL_ROW_THRESHOLD

        const atMax = maxRows !== undefined && fields.length >= maxRows
        const canRemoveBelowMin =
          schema.minItems === undefined || fields.length > schema.minItems

        // Map each Form.List field to a Table row record. We carry the
        // antd Form.List `field` so each cell renders a Form.Item bound to
        // the right `[rowName, colKey]` path; `field.key` is the stable
        // selection/react key.
        const dataSource = fields.map(f => ({ key: f.key, field: f }))
        type Row = (typeof dataSource)[number]

        // Bulk-set the bulkToggle column on the selected rows.
        const bulkSet = (value: boolean) => {
          if (!bulkCol) return
          const current =
            (form.getFieldValue(name) as Array<Record<string, unknown>>) ?? []
          const next = current.map((row, idx) => {
            const fieldForIdx = fields[idx]
            if (fieldForIdx && selectedKeys.includes(fieldForIdx.key)) {
              return { ...row, [bulkCol.key]: value }
            }
            return row
          })
          form.setFieldValue(name, next)
        }

        const bulkDelete = () => {
          // Remove from the highest index down so earlier removals don't
          // shift the indices of rows still to be removed.
          const toRemove = fields
            .filter(f => selectedKeys.includes(f.key))
            .map(f => f.name)
            .sort((a, b) => b - a)
          toRemove.forEach(n => remove(n))
          setSelectedKeys([])
        }

        const dataColumns: TableColumnsType<Row> = cols.map(col => {
          const colUi = col.field.ui ?? {}
          const column: TableColumnsType<Row>[number] = {
            title: col.field.title || col.key,
            key: col.key,
            dataIndex: col.key,
            width: colUi.width ?? DEFAULT_COL_WIDTH,
            render: (_: unknown, row: Row) => (
              <EditableCell
                rowName={row.field.name}
                col={col}
                disabled={disabled}
              />
            ),
          }
          if (colUi.sortable) {
            column.sorter = (a: Row, b: Row) => {
              const va = form.getFieldValue([name, a.field.name, col.key])
              const vb = form.getFieldValue([name, b.field.name, col.key])
              if (typeof va === 'number' && typeof vb === 'number') {
                return va - vb
              }
              return String(va ?? '').localeCompare(String(vb ?? ''))
            }
          }
          if (colUi.filterable) {
            // Build the filter set from the enum (if any) or the current
            // distinct cell values.
            const values =
              col.field.enum ??
              Array.from(
                new Set(
                  fields
                    .map(f =>
                      form.getFieldValue([name, f.name, col.key]),
                    )
                    .filter(v => v !== undefined && v !== null && v !== ''),
                ),
              ).map(v => String(v))
            column.filters = values.map(v => ({ text: String(v), value: String(v) }))
            column.onFilter = (value, row: Row) =>
              String(form.getFieldValue([name, row.field.name, col.key])) ===
              String(value)
          }
          return column
        })

        const actionColumn: TableColumnsType<Row>[number] = {
          title: '',
          key: '__actions',
          width: 48,
          fixed: 'right',
          render: (_: unknown, row: Row) => (
            <Button
              size="small"
              type="text"
              icon={<Trash2 />}
              disabled={disabled || !canRemoveBelowMin}
              aria-label="Remove row"
              onClick={() => remove(row.field.name)}
            />
          ),
        }

        const columns: TableColumnsType<Row> = [...dataColumns, actionColumn]
        const scrollX =
          cols.reduce(
            (sum, c) => sum + (c.field.ui?.width ?? DEFAULT_COL_WIDTH),
            0,
          ) + 48

        return (
          <div className="flex flex-col gap-2">
            {(bulkCol || selectedKeys.length > 0) && (
              <Space wrap>
                {bulkCol && (
                  <>
                    <Button
                      size="small"
                      disabled={disabled || selectedKeys.length === 0}
                      onClick={() => bulkSet(true)}
                    >
                      Set {bulkCol.field.title || bulkCol.key} on
                    </Button>
                    <Button
                      size="small"
                      disabled={disabled || selectedKeys.length === 0}
                      onClick={() => bulkSet(false)}
                    >
                      Set {bulkCol.field.title || bulkCol.key} off
                    </Button>
                  </>
                )}
                <Button
                  size="small"
                  danger
                  disabled={disabled || selectedKeys.length === 0}
                  onClick={bulkDelete}
                >
                  Delete selected
                </Button>
                {selectedKeys.length > 0 && (
                  <Text type="secondary" className="text-xs">
                    {selectedKeys.length} selected
                  </Text>
                )}
              </Space>
            )}

            <div
              ref={wrapRef}
              style={useVirtual ? { height: 360 } : undefined}
            >
              <Table<Row>
                size="small"
                rowKey="key"
                columns={columns}
                dataSource={dataSource}
                pagination={false}
                virtual={useVirtual}
                scroll={
                  useVirtual ? { x: scrollX, y: bodyHeight } : { x: scrollX }
                }
                rowSelection={
                  bulkCol
                    ? {
                        selectedRowKeys: selectedKeys,
                        onChange: keys => setSelectedKeys(keys),
                      }
                    : undefined
                }
                expandable={
                  expandCol
                    ? {
                        expandedRowRender: (row: Row) => (
                          <Text className="text-xs whitespace-pre-wrap">
                            {String(
                              form.getFieldValue([
                                name,
                                row.field.name,
                                expandCol.key,
                              ]) ?? '',
                            )}
                          </Text>
                        ),
                      }
                    : undefined
                }
              />
            </div>

            <Form.ErrorList errors={errors} />

            <Button
              type="dashed"
              size="small"
              block
              icon={<Plus />}
              disabled={disabled || atMax}
              onClick={() => add(newRow(cols))}
            >
              Add row{atMax ? ` (max ${maxRows})` : ''}
            </Button>
          </div>
        )
      }}
    </Form.List>
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
