import { useState } from 'react'
import {
  Alert,
  Button,
  DatePicker,
  Descriptions,
  Form,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Typography,
} from 'antd'
import {
  CircleCheck,
  CircleX,
  SquarePen,
  Ban,
} from 'lucide-react'
import dayjs, { type Dayjs } from 'dayjs'
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
  /** JSON Schema string format. Per MCP spec: email, uri, date, date-time, password. */
  format?: string
  default?: unknown
  minimum?: number
  maximum?: number
  minLength?: number
  maxLength?: number
  /** JSON Schema regex constraint for strings. */
  pattern?: string
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

function getOptions(
  fieldSchema: FieldSchema,
): { value: string; label: string }[] {
  // TitledSingleSelectEnum — anyOf or oneOf at top level of the property schema
  if (
    fieldSchema.type === 'string' &&
    (fieldSchema.anyOf || fieldSchema.oneOf)
  ) {
    const options = fieldSchema.anyOf ?? fieldSchema.oneOf!
    return options.map(o => ({ value: o.const, label: o.title ?? o.const }))
  }
  // UntitledSingleSelectEnum or LegacyEnum (enumNames as labels)
  if (fieldSchema.type === 'string' && fieldSchema.enum) {
    const names = fieldSchema.enumNames ?? fieldSchema.enum
    return fieldSchema.enum.map((v, i) => ({ value: v, label: names[i] ?? v }))
  }
  // TitledMultiSelectEnum — anyOf or oneOf inside items
  if (
    fieldSchema.type === 'array' &&
    (fieldSchema.items?.anyOf || fieldSchema.items?.oneOf)
  ) {
    const options = fieldSchema.items.anyOf ?? fieldSchema.items.oneOf!
    return options.map(o => ({ value: o.const, label: o.title ?? o.const }))
  }
  // UntitledMultiSelectEnum — enum inside items
  if (fieldSchema.type === 'array' && fieldSchema.items?.enum) {
    return fieldSchema.items.enum.map(v => ({ value: v, label: v }))
  }
  return []
}

function renderField(
  name: string,
  fieldSchema: FieldSchema,
  required: boolean,
): React.ReactNode {
  const label = fieldSchema.title || name
  const rules: object[] = required
    ? [{ required: true, message: `${label} is required` }]
    : []
  const testId = `elicitation-field-${name}`

  // Select fields (single or multi)
  const isMultiSelect =
    fieldSchema.type === 'array' &&
    !!(
      fieldSchema.items?.enum ||
      fieldSchema.items?.anyOf ||
      fieldSchema.items?.oneOf
    )
  const isSelectField =
    isMultiSelect ||
    (fieldSchema.type === 'string' &&
      !!(fieldSchema.enum || fieldSchema.anyOf || fieldSchema.oneOf))

  if (isSelectField) {
    const options = getOptions(fieldSchema)
    if (isMultiSelect) {
      if (
        fieldSchema.minItems !== undefined ||
        fieldSchema.maxItems !== undefined
      ) {
        rules.push({
          type: 'array',
          min: fieldSchema.minItems,
          max: fieldSchema.maxItems,
        })
      }
    }
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        extra={fieldSchema.description}
      >
        <Select
          options={options}
          mode={isMultiSelect ? 'multiple' : undefined}
          placeholder={`Select ${label.toLowerCase()}`}
          data-testid={testId}
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
        extra={fieldSchema.description}
      >
        <Switch data-testid={testId} />
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
        extra={fieldSchema.description}
      >
        <InputNumber
          min={fieldSchema.minimum}
          max={fieldSchema.maximum}
          precision={fieldSchema.type === 'integer' ? 0 : undefined}
          style={{ width: '100%' }}
          data-testid={testId}
        />
      </Form.Item>
    )
  }

  // ─── String formats with dedicated pickers ─────────────────────────────
  // DatePicker stores Dayjs objects in form state; handleSubmit converts to
  // ISO strings before submission so the MCP server receives the expected
  // JSON Schema `date` / `date-time` shape.
  if (fieldSchema.type === 'string' && fieldSchema.format === 'date') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        extra={fieldSchema.description}
      >
        <DatePicker
          format="YYYY-MM-DD"
          style={{ width: '100%' }}
          data-testid={testId}
        />
      </Form.Item>
    )
  }

  if (fieldSchema.type === 'string' && fieldSchema.format === 'date-time') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        extra={fieldSchema.description}
      >
        <DatePicker
          showTime
          format="YYYY-MM-DD HH:mm:ss"
          style={{ width: '100%' }}
          data-testid={testId}
        />
      </Form.Item>
    )
  }

  // ─── String constraints ────────────────────────────────────────────────
  if (
    fieldSchema.minLength !== undefined ||
    fieldSchema.maxLength !== undefined
  ) {
    rules.push({ min: fieldSchema.minLength, max: fieldSchema.maxLength })
  }

  if (fieldSchema.pattern) {
    try {
      const re = new RegExp(fieldSchema.pattern)
      rules.push({
        pattern: re,
        message: `${label} must match the required pattern`,
      })
    } catch {
      // Server sent a malformed regex — surface it as a soft validation
      // failure rather than crashing the form render.
      rules.push({
        validator: (_: unknown, value: string) =>
          value
            ? Promise.reject(
                new Error('Server sent an invalid pattern for this field'),
              )
            : Promise.resolve(),
      })
    }
  }

  if (fieldSchema.format === 'email') {
    rules.push({ type: 'email', message: 'Enter a valid email address' })
  }

  if (fieldSchema.format === 'uri') {
    rules.push({ type: 'url', message: 'Enter a valid URL' })
  }

  if (fieldSchema.format === 'password') {
    return (
      <Form.Item
        key={name}
        name={name}
        label={label}
        rules={rules}
        extra={fieldSchema.description}
      >
        <Input.Password data-testid={testId} />
      </Form.Item>
    )
  }

  const inputType =
    fieldSchema.format === 'email'
      ? 'email'
      : fieldSchema.format === 'uri'
        ? 'url'
        : 'text'

  return (
    <Form.Item
      key={name}
      name={name}
      label={label}
      rules={rules}
      extra={fieldSchema.description}
    >
      <Input type={inputType} data-testid={testId} />
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
export function ElicitationFormContent({
  content: data,
}: ContentRendererProps) {
  const [form] = Form.useForm()
  const [isSubmitting, setIsSubmitting] = useState(false)

  const elicitation = data.content as unknown as ElicitationData

  // McpStore holds the live entry while THIS device is streaming the turn; when
  // present its status is freshest. But generation now runs as a detached
  // server-side task, so an elicitation stays alive (blocked, awaiting input)
  // across a reload or on another device that has no live entry. Trust the
  // persisted DB status in that case — a `pending` block is still answerable
  // (the form submits by `elicitation_id`); only show cancelled/declined/accepted
  // when the DB itself records that terminal state.
  const mcpEntry = Stores.McpComposer.elicitationRequests.get(
    elicitation.elicitation_id,
  )
  const status = mcpEntry?.status ?? elicitation.status ?? 'pending'
  const responseContent =
    mcpEntry?.response_content ?? elicitation.response_content

  const schema = elicitation.requested_schema
  const properties = schema?.properties || {}
  const requiredFields = new Set(schema?.required || [])

  // Date/date-time defaults come from the server as ISO strings; convert to
  // dayjs so AntD DatePicker can display them. Other defaults pass through.
  const initialValues = Object.fromEntries(
    Object.entries(properties).map(([key, field]) => {
      const fs = field as FieldSchema
      const def = fs.default
      if (
        typeof def === 'string' &&
        fs.type === 'string' &&
        (fs.format === 'date' || fs.format === 'date-time')
      ) {
        const d = dayjs(def)
        return [key, d.isValid() ? d : undefined]
      }
      return [key, def]
    }),
  )

  const handleSubmit = async () => {
    let values: Record<string, unknown>
    try {
      values = await form.validateFields()
    } catch {
      // Validation failed — form shows inline errors, stay interactive
      return
    }
    setIsSubmitting(true)
    try {
      // Convert dayjs values back to ISO strings per the field's schema format
      // so the MCP server receives the canonical JSON Schema representation.
      const submitValues: Record<string, unknown> = {}
      for (const [key, val] of Object.entries(values)) {
        const fs = properties[key] as FieldSchema | undefined
        if (val != null && dayjs.isDayjs(val)) {
          const d = val as Dayjs
          submitValues[key] =
            fs?.format === 'date' ? d.format('YYYY-MM-DD') : d.toISOString()
        } else {
          submitValues[key] = val
        }
      }
      await Stores.McpComposer.resolveElicitation(
        elicitation.elicitation_id,
        'accept',
        submitValues,
      )
    } catch (e) {
      // The store rolls status back to 'pending' on POST failure so the
      // user can retry; swallow here so the error doesn't bubble to the
      // chat error boundary.
      console.warn('mcp.elicitation resolve failed', e)
    } finally {
      // On success the resolved card replaces this form (no-op); on
      // failure the catch above kept us interactive — either way,
      // make sure the submit button is re-enabled.
      setIsSubmitting(false)
    }
  }

  const handleDecline = async () => {
    setIsSubmitting(true)
    try {
      await Stores.McpComposer.resolveElicitation(elicitation.elicitation_id, 'decline')
    } finally {
      setIsSubmitting(false)
    }
  }

  // --- Resolved states ---

  if (status === 'accepted') {
    const items = responseContent
      ? Object.entries(responseContent).map(([key, value]) => {
          const fieldSchema = properties[key] as FieldSchema | undefined
          const label = fieldSchema?.title || key
          return {
            key,
            label,
            children: Array.isArray(value)
              ? value.join(', ')
              : String(value ?? ''),
          }
        })
      : []

    return (
      <div
        className="my-2"
        data-testid={`elicitation-accepted-${elicitation.elicitation_id}`}
      >
        <Alert
          type="success"
          icon={<CircleCheck />}
          showIcon
          title={
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
      <div
        className="my-2"
        data-testid={`elicitation-declined-${elicitation.elicitation_id}`}
      >
        <Alert
          type="warning"
          icon={<CircleX />}
          showIcon
          title={
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
      <div
        className="my-2"
        data-testid={`elicitation-cancelled-${elicitation.elicitation_id}`}
      >
        <Alert
          type="error"
          icon={<Ban />}
          showIcon
          title={
            <div>
              <Text strong>{elicitation.server}</Text>
              <Text type="secondary" className="ml-2 text-xs">
                — session expired
              </Text>
            </div>
          }
          description="This form can no longer be submitted — the request expired or was cancelled."
        />
      </div>
    )
  }

  // --- Pending state: interactive form ---
  return (
    <div
      className="my-2"
      data-testid={`elicitation-pending-${elicitation.elicitation_id}`}
    >
      <Alert
        type="info"
        icon={<SquarePen />}
        showIcon
        title={
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
                renderField(
                  name,
                  fieldSchema as FieldSchema,
                  requiredFields.has(name),
                ),
              )}
            </Form>
            <Space className="mt-2">
              <Button
                type="primary"
                onClick={handleSubmit}
                loading={isSubmitting}
                size="small"
                data-testid="elicitation-submit"
              >
                Submit
              </Button>
              <Button
                onClick={handleDecline}
                loading={isSubmitting}
                size="small"
                data-testid="elicitation-decline"
              >
                Decline
              </Button>
            </Space>
          </div>
        }
      />
    </div>
  )
}
