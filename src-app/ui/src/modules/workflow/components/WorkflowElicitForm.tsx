import { FormOutlined } from '@ant-design/icons'
import {
  Alert,
  Button,
  Form,
  Input,
  InputNumber,
  Select,
  Switch,
  Typography,
} from 'antd'
import type { FormInstance } from 'antd/es/form'
import { useState } from 'react'
import type { SSEElicitationRequiredData } from '@/api-client/types'
import { EditableArrayTable } from './EditableArrayTable'
import {
  type ElicitObjectSchema,
  type FieldSchema,
  fieldRules,
  isTableProperty,
  listRules,
  validateTableValue,
} from './workflowElicitSchema'

const { Text } = Typography

interface WorkflowElicitFormProps {
  elicitation: SSEElicitationRequiredData
  submitting: boolean
  onSubmit: (response: Record<string, unknown>) => void
}

function renderField(
  name: string,
  field: FieldSchema,
  required: boolean,
  form: FormInstance,
  disabled: boolean,
): React.ReactNode {
  const label = field.title || name
  const rules = fieldRules(field, required, label)

  // Array-of-objects (or an explicit ui.widget==='table') → editable table.
  // The table's own `Form.List name={name}` owns the binding + the
  // array-level (minItems/maxItems) validation, so we render only the
  // label/description chrome here (a binding Form.Item would double-bind
  // the same path). Per-cell rules live inside the table.
  if (isTableProperty(field)) {
    return (
      <div key={name} className="mb-4">
        <div className="mb-1">
          <Text className="text-sm">{label}</Text>
        </div>
        {field.description && (
          <Text type="secondary" className="text-xs block mb-1">
            {field.description}
          </Text>
        )}
        <EditableArrayTable
          name={name}
          schema={field}
          form={form}
          disabled={disabled}
          listRules={listRules(field, required, label)}
        />
      </div>
    )
  }

  if (field.enum) {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules.length > 0 ? rules : undefined}
        extra={field.description}
      >
        <Select
          options={field.enum.map(v => ({ value: v, label: String(v) }))}
        />
      </Form.Item>
    )
  }
  if (field.type === 'boolean') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        valuePropName="checked"
        extra={field.description}
        rules={rules.length > 0 ? rules : undefined}
      >
        <Switch />
      </Form.Item>
    )
  }
  if (field.type === 'number' || field.type === 'integer') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules.length > 0 ? rules : undefined}
        extra={field.description}
      >
        <InputNumber
          min={field.minimum}
          max={field.maximum}
          precision={field.type === 'integer' ? 0 : undefined}
          style={{ width: '100%' }}
        />
      </Form.Item>
    )
  }
  return (
    <Form.Item
      key={name}
      name={name}
      label={label}
      rules={rules.length > 0 ? rules : undefined}
      extra={field.description}
    >
      <Input />
    </Form.Item>
  )
}

/**
 * Renders the JSON-schema form for a `kind: elicit` step that is
 * awaiting user input, and POSTs the response back via the parent.
 */
export function WorkflowElicitForm({
  elicitation,
  submitting,
  onSubmit,
}: WorkflowElicitFormProps) {
  const [form] = Form.useForm()
  const schema = (elicitation.schema ?? {}) as ElicitObjectSchema
  const properties = schema.properties ?? {}
  const required = new Set(schema.required ?? [])

  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      // E5 parity for VIRTUAL tables: `validateFields()` only checks MOUNTED
      // cells, so off-screen rows of a virtualized table escape per-cell rules.
      // Re-check each table property's full (Form.List-preserved) array.
      for (const [name, field] of Object.entries(properties)) {
        if (isTableProperty(field)) {
          const err = validateTableValue(
            (values as Record<string, unknown>)[name],
            field.items,
            field.title || name,
          )
          if (err) {
            setError(err)
            return
          }
        }
      }
      setError(null)
      onSubmit(values)
    } catch {
      setError('Please fix the highlighted fields')
    }
  }

  // Seed initial values from each property's `default`, but PREFER a value
  // supplied in `elicitation.data` (keyed by property name) when present.
  // Keeps working when `data` is absent.
  //
  // A seeded ARRAY may contain `null` elements — e.g. an `llm_map` step's
  // `on_error: skip` materializes a skipped item as null. A null has no editable
  // table row and its required cells would silently block submit, so drop nulls
  // from array seeds (synthesis/downstream consumers already ignore them).
  const seed = (elicitation.data ?? {}) as Record<string, unknown>
  const seedValue = (k: string, fallback: unknown) => {
    if (!(k in seed)) return fallback
    const v = seed[k]
    return Array.isArray(v) ? v.filter(el => el !== null && el !== undefined) : v
  }
  const initialValues = Object.fromEntries(
    Object.entries(properties).map(([k, f]) => [k, seedValue(k, f.default)]),
  )

  return (
    <Alert
      type="info"
      icon={<FormOutlined />}
      showIcon
      title="Input required"
      description={
        <div className="mt-2">
          <Text className="text-sm">{elicitation.message}</Text>
          <Form
            form={form}
            layout="vertical"
            className="mt-3"
            initialValues={initialValues}
            disabled={submitting}
          >
            {Object.entries(properties).map(([name, field]) =>
              renderField(name, field, required.has(name), form, submitting),
            )}
          </Form>
          {error && (
            <Alert type="error" title={error} showIcon className="!mb-2" />
          )}
          <Button
            type="primary"
            size="small"
            loading={submitting}
            onClick={handleSubmit}
          >
            Submit
          </Button>
        </div>
      }
    />
  )
}
