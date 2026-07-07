import { useState } from 'react'
import { z } from 'zod'
import { EMAIL_RE } from '@/lib/validation'
import {
  Alert,
  Button,
  DatePicker,
  Descriptions,
  Form,
  FormField,
  Input,
  PasswordInput,
  InputNumber,
  MultiSelect,
  Select,
  Space,
  Switch,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import {
  CircleCheck,
  CircleX,
  SquarePen,
  Ban,
} from 'lucide-react'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'

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

/** Build a zod schema for a single field. */
function buildFieldZodSchema(fieldSchema: FieldSchema, required: boolean): z.ZodTypeAny {
  const label = fieldSchema.title ?? 'This field'
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

  let schema: z.ZodTypeAny

  if (isMultiSelect) {
    let s = z.array(z.string())
    if (fieldSchema.minItems != null)
      s = s.min(fieldSchema.minItems, `Select at least ${fieldSchema.minItems} item(s)`)
    if (fieldSchema.maxItems != null)
      s = s.max(fieldSchema.maxItems, `Select at most ${fieldSchema.maxItems} item(s)`)
    schema = required
      ? z.preprocess((v) => v ?? [], s.min(1, `${label} is required`))
      : s.optional()
    return schema
  }

  if (isSelectField) {
    schema = required
      ? z.preprocess((v) => v ?? '', z.string().min(1, `${label} is required`))
      : z.string().optional()
    return schema
  }

  if (fieldSchema.type === 'boolean') {
    schema = z.boolean()
    return required ? schema : schema.optional()
  }

  if (fieldSchema.type === 'number' || fieldSchema.type === 'integer') {
    let s = z.number({ error: `${label} must be a number` })
    if (fieldSchema.type === 'integer') s = s.int(`${label} must be a whole number`)
    if (fieldSchema.minimum != null) s = s.min(fieldSchema.minimum, `${label} must be at least ${fieldSchema.minimum}`)
    if (fieldSchema.maximum != null) s = s.max(fieldSchema.maximum, `${label} must be at most ${fieldSchema.maximum}`)
    schema = required ? s : s.optional()
    return schema
  }

  // String (including date / date-time / email / uri / password)
  let s = z.string()
  if (fieldSchema.minLength != null)
    s = s.min(fieldSchema.minLength, `${label} must be at least ${fieldSchema.minLength} character(s)`)
  if (fieldSchema.maxLength != null)
    s = s.max(fieldSchema.maxLength, `${label} must be at most ${fieldSchema.maxLength} character(s)`)
  if (fieldSchema.pattern) {
    try {
      s = s.regex(new RegExp(fieldSchema.pattern), `${label} must match the required pattern`)
    } catch {
      // Server sent a malformed regex — skip the constraint rather than crashing.
    }
  }
  if (fieldSchema.format === 'email') s = s.regex(EMAIL_RE, 'Enter a valid email address')
  if (fieldSchema.format === 'uri') s = s.url('Enter a valid URL')

  // A required field left untouched holds `undefined` (its default), which would
  // otherwise fail with zod's raw type error ("expected string, received
  // undefined") instead of the intended "<label> is required". Coerce nullish →
  // '' first so `min(1)` produces the friendly required message. Only applied to
  // required fields, so a successful (non-empty) submit is unaffected.
  schema = required
    ? z.preprocess((v) => v ?? '', s.min(1, `${label} is required`))
    : s.optional()
  return schema
}

/** Build a zod object schema from all property schemas. */
function buildFormSchema(
  properties: Record<string, FieldSchema>,
  requiredFields: Set<string>,
): z.ZodObject<Record<string, z.ZodTypeAny>> {
  const shape: Record<string, z.ZodTypeAny> = {}
  for (const [name, fieldSchema] of Object.entries(properties)) {
    shape[name] = buildFieldZodSchema(fieldSchema as FieldSchema, requiredFields.has(name))
  }
  return z.object(shape)
}

function renderField(
  name: string,
  fieldSchema: FieldSchema,
  required: boolean,
): React.ReactNode {
  const label = fieldSchema.title || name
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
      return (
        <FormField
          key={name}
          name={name}
          label={label}
          required={required}
          description={fieldSchema.description}
        >
          <MultiSelect
            options={options}
            placeholder={`Select ${label.toLowerCase()}`}
            searchPlaceholder="Search…"
            emptyText="No options"
            removeLabel={v => `Remove ${v}`}
            aria-label={label}
            data-testid={testId}
          />
        </FormField>
      )
    }
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <Select
          options={options}
          placeholder={`Select ${label.toLowerCase()}`}
          data-testid={testId}
        />
      </FormField>
    )
  }

  if (fieldSchema.type === 'boolean') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        valuePropName="checked"
        description={fieldSchema.description}
      >
        <Switch data-testid={testId} />
      </FormField>
    )
  }

  if (fieldSchema.type === 'number' || fieldSchema.type === 'integer') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <InputNumber
          min={fieldSchema.minimum}
          max={fieldSchema.maximum}
          precision={fieldSchema.type === 'integer' ? 0 : undefined}
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  // ─── String formats with dedicated pickers ─────────────────────────────
  // DatePicker stores an ISO string in form state; no dayjs conversion needed.
  // NOTE: the kit DatePicker is date-only (no showTime); date-time fields get
  // date-only selection and the time component will be T00:00:00 in the emitted
  // ISO string. See FLAG: DatePicker showTime below.
  if (fieldSchema.type === 'string' && fieldSchema.format === 'date') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <DatePicker
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd"
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  if (fieldSchema.type === 'string' && fieldSchema.format === 'date-time') {
    // FLAG: kit DatePicker has no showTime — time will be T00:00:00 in the
    // emitted value. Full datetime picking requires a future kit component.
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <DatePicker
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd'T'HH:mm:ss"
          className="w-full"
          data-testid={testId}
        />
      </FormField>
    )
  }

  if (fieldSchema.format === 'password') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={fieldSchema.description}
      >
        <PasswordInput showLabel="Show" hideLabel="Hide" data-testid={testId} />
      </FormField>
    )
  }

  const inputType =
    fieldSchema.format === 'email'
      ? 'email'
      : fieldSchema.format === 'uri'
        ? 'url'
        : 'text'

  return (
    <FormField
      key={name}
      name={name}
      label={label}
      required={required}
      description={fieldSchema.description}
    >
      <Input type={inputType} data-testid={testId} />
    </FormField>
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

  // Build a dynamic zod schema from the elicitation field specs.
  const formSchema = buildFormSchema(properties, requiredFields)

  // Date/date-time defaults come from the server as ISO strings; the kit
  // DatePicker accepts ISO strings directly — no dayjs conversion needed.
  const defaultValues = Object.fromEntries(
    Object.entries(properties).map(([key, field]) => {
      const fs = field as FieldSchema
      return [key, fs.default ?? undefined]
    }),
  )

  const form = useForm<Record<string, unknown>>({
    resolver: zodResolver(formSchema),
    defaultValues,
  })

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
          tone="success"
          data-testid="mcp-elicitation-accepted-alert"
          icon={<CircleCheck />}
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
                size="sm"
                column={1}
                items={items}
                className="mt-2"
                data-testid="mcp-elicitation-summary"
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
          tone="warning"
          data-testid="mcp-elicitation-declined-alert"
          icon={<CircleX />}
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
          tone="error"
          data-testid="mcp-elicitation-cancelled-alert"
          icon={<Ban />}
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
        tone="info"
        data-testid="mcp-elicitation-pending-alert"
        icon={<SquarePen />}
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
              className="mt-3"
              disabled={isSubmitting}
              data-testid="mcp-elicitation-form"
              onSubmit={async (values) => {
                setIsSubmitting(true)
                try {
                  await Stores.McpComposer.resolveElicitation(
                    elicitation.elicitation_id,
                    'accept',
                    values as Record<string, unknown>,
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
              }}
            >
              {Object.entries(properties).map(([name, fieldSchema]) =>
                renderField(
                  name,
                  fieldSchema as FieldSchema,
                  requiredFields.has(name),
                ),
              )}
              <Space className="mt-2">
                <Button
                  type="submit"
                  loading={isSubmitting}
                  size="default"
                  data-testid="elicitation-submit"
                >
                  Submit
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleDecline}
                  loading={isSubmitting}
                  size="default"
                  data-testid="elicitation-decline"
                >
                  Decline
                </Button>
              </Space>
            </Form>
          </div>
        }
      />
    </div>
  )
}
