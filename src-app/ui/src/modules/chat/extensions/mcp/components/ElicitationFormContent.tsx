import { useState } from 'react'
import { Alert, Button, Descriptions, Form, Input, InputNumber, Select, Space, Switch, Typography } from 'antd'
import { CheckCircleOutlined, CloseCircleOutlined, FormOutlined, StopOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'

const { Text } = Typography

interface ElicitationData {
  type: 'elicitation_request'
  elicitation_id: string
  message_id?: string
  message: string
  requested_schema: {
    type: 'object'
    properties: Record<string, FieldSchema>
    required?: string[]
  }
  server: string
  /** "pending" | "accepted" | "declined" | "cancelled" — persisted in DB */
  status?: string
  /** Submitted field values (only present when status = "accepted") */
  response_content?: Record<string, unknown>
}

interface FieldSchema {
  type?: string
  title?: string
  description?: string
  format?: string
  default?: unknown
  minimum?: number
  maximum?: number
  minLength?: number
  maxLength?: number
  minItems?: number
  maxItems?: number
  enum?: string[]
  enumNames?: string[]
  anyOf?: Array<{ const: string; title?: string }>
  oneOf?: Array<{ const: string; title?: string }>
  items?: {
    type?: string
    enum?: string[]
    anyOf?: Array<{ const: string; title?: string }>
    oneOf?: Array<{ const: string; title?: string }>
  }
}

function getOptions(fieldSchema: FieldSchema): { value: string; label: string }[] {
  // TitledSingleSelectEnum — anyOf or oneOf at top level of the property schema
  if (fieldSchema.type === 'string' && (fieldSchema.anyOf || fieldSchema.oneOf)) {
    const options = fieldSchema.anyOf ?? fieldSchema.oneOf!
    return options.map(o => ({ value: o.const, label: o.title ?? o.const }))
  }
  // UntitledSingleSelectEnum or LegacyEnum (enumNames as labels)
  if (fieldSchema.type === 'string' && fieldSchema.enum) {
    const names = fieldSchema.enumNames ?? fieldSchema.enum
    return fieldSchema.enum.map((v, i) => ({ value: v, label: names[i] ?? v }))
  }
  // TitledMultiSelectEnum — anyOf or oneOf inside items
  if (fieldSchema.type === 'array' && (fieldSchema.items?.anyOf || fieldSchema.items?.oneOf)) {
    const options = fieldSchema.items.anyOf ?? fieldSchema.items.oneOf!
    return options.map(o => ({ value: o.const, label: o.title ?? o.const }))
  }
  // UntitledMultiSelectEnum — enum inside items
  if (fieldSchema.type === 'array' && fieldSchema.items?.enum) {
    return fieldSchema.items.enum.map(v => ({ value: v, label: v }))
  }
  return []
}

function renderField(name: string, fieldSchema: FieldSchema, required: boolean): React.ReactNode {
  const label = fieldSchema.title || name
  const rules: object[] = required ? [{ required: true, message: `${label} is required` }] : []

  // Select fields (single or multi)
  const isMultiSelect =
    fieldSchema.type === 'array' &&
    !!(fieldSchema.items?.enum || fieldSchema.items?.anyOf || fieldSchema.items?.oneOf)
  const isSelectField =
    isMultiSelect ||
    (fieldSchema.type === 'string' && !!(fieldSchema.enum || fieldSchema.anyOf || fieldSchema.oneOf))

  if (isSelectField) {
    const options = getOptions(fieldSchema)
    if (isMultiSelect) {
      if (fieldSchema.minItems !== undefined || fieldSchema.maxItems !== undefined) {
        rules.push({ type: 'array', min: fieldSchema.minItems, max: fieldSchema.maxItems })
      }
    }
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        tooltip={fieldSchema.description}
      >
        <Select
          options={options}
          mode={isMultiSelect ? 'multiple' : undefined}
          placeholder={`Select ${label.toLowerCase()}`}
        />
      </Form.Item>
    )
  }

  if (fieldSchema.type === 'boolean') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        valuePropName="checked"
        tooltip={fieldSchema.description}
      >
        <Switch />
      </Form.Item>
    )
  }

  if (fieldSchema.type === 'number' || fieldSchema.type === 'integer') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        tooltip={fieldSchema.description}
      >
        <InputNumber
          min={fieldSchema.minimum}
          max={fieldSchema.maximum}
          precision={fieldSchema.type === 'integer' ? 0 : undefined}
          style={{ width: '100%' }}
        />
      </Form.Item>
    )
  }

  // Default: string
  if (fieldSchema.minLength !== undefined || fieldSchema.maxLength !== undefined) {
    rules.push({ min: fieldSchema.minLength, max: fieldSchema.maxLength })
  }

  if (fieldSchema.format === 'password') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        tooltip={fieldSchema.description}
      >
        <Input.Password />
      </Form.Item>
    )
  }

  return (
    <Form.Item
      key={name}
      name={name}
      label={label}
      rules={rules}
      tooltip={fieldSchema.description}
    >
      <Input type={fieldSchema.format === 'email' ? 'email' : 'text'} />
    </Form.Item>
  )
}

/**
 * ElicitationFormContent
 *
 * Renders a dynamic form for MCP server elicitation requests.
 * Supports all SEP-1330 field types: primitives, single-select, multi-select enums.
 *
 * Four states:
 * - pending: interactive form (submittable)
 * - accepted: read-only display of submitted values
 * - declined: declined notice
 * - cancelled: session expired notice
 */
export function ElicitationFormContent({ content: data }: ContentRendererProps) {
  const [form] = Form.useForm()
  const [isSubmitting, setIsSubmitting] = useState(false)

  const elicitation = data.content as unknown as ElicitationData

  // McpStore is the live source of truth during streaming.
  // After page reload the store is empty; a pending status from DB means the session ended
  // without the user responding, so treat it as cancelled (the backend already cancelled it).
  const mcpEntry = Stores.Chat.__state.McpStore.elicitationRequests.get(elicitation.elicitation_id)
  const isLive = mcpEntry != null
  const status = isLive
    ? (mcpEntry.status ?? elicitation.status ?? 'pending')
    : (elicitation.status === 'pending' ? 'cancelled' : (elicitation.status ?? 'pending'))
  const responseContent = mcpEntry?.response_content ?? elicitation.response_content

  const schema = elicitation.requested_schema
  const properties = schema?.properties || {}
  const requiredFields = new Set(schema?.required || [])

  const initialValues = Object.fromEntries(
    Object.entries(properties).map(([key, field]) => [key, (field as FieldSchema).default])
  )

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      setIsSubmitting(true)
      const mcpStore = Stores.Chat.__state.McpStore
      await mcpStore.resolveElicitation(elicitation.elicitation_id, 'accept', values)
    } catch {
      // Validation failed — form shows inline errors, stay interactive
      setIsSubmitting(false)
    }
  }

  const handleDecline = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.Chat.__state.McpStore
    await mcpStore.resolveElicitation(elicitation.elicitation_id, 'decline')
  }

  // --- Resolved states ---

  if (status === 'accepted') {
    const items = responseContent
      ? Object.entries(responseContent).map(([key, value]) => {
          const fieldSchema = properties[key] as FieldSchema | undefined
          const label = fieldSchema?.title || key
          return { key, label, children: Array.isArray(value) ? value.join(', ') : String(value ?? '') }
        })
      : []

    return (
      <div className="my-2">
        <Alert
          type="success"
          icon={<CheckCircleOutlined />}
          showIcon
          message={
            <div>
              <Text strong>{elicitation.server}</Text>
              <Text type="secondary" className="ml-2 text-xs">
                — form submitted
              </Text>
            </div>
          }
          description={
            items.length > 0 ? (
              <Descriptions
                size="small"
                column={1}
                items={items}
                className="mt-2"
              />
            ) : null
          }
        />
      </div>
    )
  }

  if (status === 'declined') {
    return (
      <div className="my-2">
        <Alert
          type="warning"
          icon={<CloseCircleOutlined />}
          showIcon
          message={
            <div>
              <Text strong>{elicitation.server}</Text>
              <Text type="secondary" className="ml-2 text-xs">
                — request declined
              </Text>
            </div>
          }
        />
      </div>
    )
  }

  if (status === 'cancelled') {
    return (
      <div className="my-2">
        <Alert
          type="error"
          icon={<StopOutlined />}
          showIcon
          message={
            <div>
              <Text strong>{elicitation.server}</Text>
              <Text type="secondary" className="ml-2 text-xs">
                — session expired
              </Text>
            </div>
          }
          description="This form can no longer be submitted. The MCP server session has ended."
        />
      </div>
    )
  }

  // --- Pending state: interactive form ---
  return (
    <div className="my-2">
      <Alert
        type="info"
        icon={<FormOutlined />}
        showIcon
        message={
          <div>
            <Text strong>{elicitation.server}</Text>
            <Text type="secondary" className="ml-2 text-xs">
              is requesting input
            </Text>
          </div>
        }
        description={
          <div className="mt-2">
            <Text className="text-sm">{elicitation.message}</Text>
            <Form
              form={form}
              layout="vertical"
              initialValues={initialValues}
              className="mt-3"
              disabled={isSubmitting}
            >
              {Object.entries(properties).map(([name, fieldSchema]) =>
                renderField(name, fieldSchema as FieldSchema, requiredFields.has(name))
              )}
            </Form>
            <Space className="mt-2">
              <Button
                type="primary"
                onClick={handleSubmit}
                loading={isSubmitting}
                size="small"
              >
                Submit
              </Button>
              <Button onClick={handleDecline} loading={isSubmitting} size="small">
                Decline
              </Button>
            </Space>
          </div>
        }
      />
    </div>
  )
}
