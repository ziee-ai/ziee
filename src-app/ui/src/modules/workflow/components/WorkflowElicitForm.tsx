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
import { useState } from 'react'
import type { SSEElicitationRequiredData } from '@/api-client/types'

const { Text } = Typography

interface FieldSchema {
  type?: string
  title?: string
  description?: string
  default?: unknown
  minimum?: number
  maximum?: number
  enum?: string[]
}

interface WorkflowElicitFormProps {
  elicitation: SSEElicitationRequiredData
  submitting: boolean
  onSubmit: (response: Record<string, unknown>) => void
}

function renderField(
  name: string,
  field: FieldSchema,
  required: boolean,
): React.ReactNode {
  const label = field.title || name
  const rules = required
    ? [{ required: true, message: `${label} is required` }]
    : undefined

  if (field.enum) {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        extra={field.description}
      >
        <Select options={field.enum.map(v => ({ value: v, label: v }))} />
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
        rules={rules}
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
      rules={rules}
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
  const schema = (elicitation.schema ?? {}) as {
    properties?: Record<string, FieldSchema>
    required?: string[]
  }
  const properties = schema.properties ?? {}
  const required = new Set(schema.required ?? [])

  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      setError(null)
      onSubmit(values)
    } catch {
      setError('Please fix the highlighted fields')
    }
  }

  const initialValues = Object.fromEntries(
    Object.entries(properties).map(([k, f]) => [k, f.default]),
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
              renderField(name, field, required.has(name)),
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
