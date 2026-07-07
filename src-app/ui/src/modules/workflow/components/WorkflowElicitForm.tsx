import { PenLine } from 'lucide-react'
import {
  Alert,
  Button,
  DatePicker,
  Form,
  FormField,
  Input,
  InputNumber,
  MultiSelect,
  PasswordInput,
  Select,
  Switch,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { useState } from 'react'
import { EMAIL_RE } from '@/lib/validation'
import type { SSEElicitationRequiredData } from '@/api-client/types'
import { EditableArrayTable } from './EditableArrayTable'
import {
  type ElicitObjectSchema,
  type FieldSchema,
  isTableProperty,
  validateTableValue,
} from './workflowElicitSchema'

// ---------------------------------------------------------------------------
// Select-option + field-kind helpers (parity with the MCP elicitation form's
// kit port in mcp/chat-extension/components/ElicitationFormContent.tsx).
// ---------------------------------------------------------------------------

/** A primitive array whose items are enum/anyOf/oneOf → renders a multi-Select.
 *  (Object-item arrays are tables — caught earlier by `isTableProperty`.) */
function isMultiSelectField(field: FieldSchema): boolean {
  return (
    field.type === 'array' &&
    !!(field.items?.enum || field.items?.anyOf || field.items?.oneOf)
  )
}

/** Titled single-select: top-level anyOf/oneOf. (`enum` keeps its own path.) */
function isTitledSelectField(field: FieldSchema): boolean {
  return !!(field.anyOf || field.oneOf)
}

/** {value,label} options for a single- or multi-select field. */
function selectOptions(field: FieldSchema): { value: string; label: string }[] {
  if (field.anyOf || field.oneOf) {
    const choices = field.anyOf ?? field.oneOf ?? []
    return choices.map(c => ({ value: c.const, label: c.title ?? c.const }))
  }
  const items = field.items
  if (items?.anyOf || items?.oneOf) {
    const choices = items.anyOf ?? items.oneOf ?? []
    return choices.map(c => ({ value: c.const, label: c.title ?? c.const }))
  }
  if (items?.enum) {
    return items.enum.map(v => ({ value: String(v), label: String(v) }))
  }
  return []
}

interface WorkflowElicitFormProps {
  elicitation: SSEElicitationRequiredData
  submitting: boolean
  onSubmit: (response: Record<string, unknown>) => void
}

// ---------------------------------------------------------------------------
// Dynamic zod schema construction from a JSON-schema ElicitObjectSchema.
// Handles: string, number, integer, boolean, enum, array-of-objects (table).
// Per-cell validation for table properties is encoded in the nested zod
// object so rhf validates cells inline as the user types.
// ---------------------------------------------------------------------------

function buildFieldZodType(field: FieldSchema, required: boolean): z.ZodTypeAny {
  const label = field.title || 'This field'

  // Array-of-objects → per-column zod shape so per-cell errors surface inline.
  // Checked FIRST so a table array is never mistaken for a multi-select.
  if (isTableProperty(field)) {
    const f = field as unknown as {
      items?: {
        properties?: Record<string, FieldSchema>
        required?: string[]
      }
      minItems?: number
      maxItems?: number
    }
    const itemProps = f.items?.properties ?? {}
    const itemRequired = new Set<string>(f.items?.required ?? [])
    const itemShape: Record<string, z.ZodTypeAny> = {}
    for (const [k, v] of Object.entries(itemProps)) {
      itemShape[k] = buildFieldZodType(v, itemRequired.has(k))
    }
    let arr = z.array(z.object(itemShape).passthrough())
    if (f.minItems !== undefined && f.minItems > 0) arr = arr.min(f.minItems)
    if (f.maxItems !== undefined) arr = arr.max(f.maxItems)
    // Arrays are always present (even if empty); mark optional only when not required.
    return required ? arr : arr.optional()
  }

  // Primitive multi-select array (enum/anyOf/oneOf items) → string[].
  if (isMultiSelectField(field)) {
    let s = z.array(z.string())
    if (field.minItems !== undefined && field.minItems > 0) {
      s = s.min(field.minItems, `Select at least ${field.minItems} item(s)`)
    }
    if (field.maxItems !== undefined) {
      s = s.max(field.maxItems, `Select at most ${field.maxItems} item(s)`)
    }
    return required
      ? s.min(Math.max(1, field.minItems ?? 1), `${label} is required`)
      : s.optional()
  }

  // Titled single-select (anyOf/oneOf): values are the string `const`s.
  if (isTitledSelectField(field)) {
    return required
      ? z.string().min(1, `${label} is required`)
      : z.string().optional().or(z.literal(''))
  }

  let schema: z.ZodTypeAny

  if (field.enum) {
    // enum: allow any of the listed values (custom validator handles any length)
    const allowed = field.enum as (string | number)[]
    schema = z.unknown().refine(v => allowed.includes(v as string | number), {
      message: `Must be one of: ${allowed.join(', ')}`,
    })
  } else if (field.type === 'boolean') {
    schema = z.boolean()
  } else if (field.type === 'number' || field.type === 'integer') {
    let s = z.number()
    if (field.type === 'integer') s = s.int()
    if (field.minimum !== undefined) s = s.min(field.minimum)
    if (field.maximum !== undefined) s = s.max(field.maximum)
    schema = s
  } else {
    // String, including date / date-time / email / uri / password formats.
    // The kit DatePicker / Input emit serializable strings into form state,
    // so no Dayjs→ISO conversion is needed at submit (unlike the antd original).
    let s = z.string()
    if (field.minLength !== undefined) {
      s = s.min(field.minLength, `${label} must be at least ${field.minLength} character(s)`)
    }
    if (field.maxLength !== undefined) {
      s = s.max(field.maxLength, `${label} must be at most ${field.maxLength} character(s)`)
    }
    if (field.pattern) {
      try {
        s = s.regex(new RegExp(field.pattern), `${label} must match the required pattern`)
      } catch {
        // Server sent a malformed regex — skip rather than crashing the form.
      }
    }
    if (field.format === 'email') s = s.regex(EMAIL_RE, 'Enter a valid email address')
    if (field.format === 'uri') s = s.url('Enter a valid URL')
    // Presence: required enforces non-empty; optional tolerates '' / cleared.
    return required
      ? s.min(1, `${label} is required`)
      : s.optional().or(z.literal(''))
  }

  // Optional fields accept undefined (and null from cleared controls).
  if (!required) {
    return schema.optional().nullable() as z.ZodTypeAny
  }
  return schema
}

function buildElicitZodSchema(
  properties: Record<string, FieldSchema>,
  required: Set<string>,
): z.ZodObject<Record<string, z.ZodTypeAny>> {
  const shape: Record<string, z.ZodTypeAny> = {}
  for (const [k, f] of Object.entries(properties)) {
    shape[k] = buildFieldZodType(f, required.has(k))
  }
  return z.object(shape)
}

// ---------------------------------------------------------------------------
// renderField — produces the kit FormField markup for each property.
// Table properties render label/description chrome only; the editable rows
// live in <EditableArrayTable> which owns the <FormList name> binding.
// ---------------------------------------------------------------------------

function renderField(
  name: string,
  field: FieldSchema,
  required: boolean,
  disabled: boolean,
): React.ReactNode {
  const label = field.title || name

  // Array-of-objects (or an explicit ui.widget==='table') → editable table.
  // The table's own `<FormList name={name}>` owns the binding + the
  // array-level (minItems/maxItems) validation, so we render only the
  // label/description chrome here (a binding FormField would double-bind
  // the same path). Per-cell rules live inside the table via the zod schema.
  if (isTableProperty(field)) {
    return (
      <div key={name} className="mb-4">
        <div className="mb-1">
          <Text className="text-sm">{label}</Text>
        </div>
        {field.description && (
          <Text className="text-xs block mb-1 text-muted-foreground">
            {field.description}
          </Text>
        )}
        <EditableArrayTable
          name={name}
          schema={field as any}
          disabled={disabled}
        />
      </div>
    )
  }

  const description = field.description ?? undefined

  // Primitive multi-select array (enum/anyOf/oneOf items) → kit MultiSelect.
  if (isMultiSelectField(field)) {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <MultiSelect
          data-testid={`wf-elicit-multiselect-${name}`}
          options={selectOptions(field)}
          placeholder={`Select ${label.toLowerCase()}`}
          searchPlaceholder="Search…"
          emptyText="No options"
          removeLabel={v => `Remove ${v}`}
          aria-label={label}
        />
      </FormField>
    )
  }

  // Titled single-select (anyOf/oneOf) → kit Select with titled choices.
  if (isTitledSelectField(field)) {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <Select
          data-testid={`wf-elicit-anyof-${name}`}
          options={selectOptions(field)}
          placeholder={`Select ${label.toLowerCase()}`}
        />
      </FormField>
    )
  }

  if (field.enum) {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <Select
          data-testid={`wf-elicit-select-${name}`}
          options={field.enum.map(v => ({ value: String(v), label: String(v) }))}
        />
      </FormField>
    )
  }
  if (field.type === 'boolean') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
        valuePropName="checked"
      >
        <Switch data-testid={`wf-elicit-switch-${name}`} />
      </FormField>
    )
  }
  if (field.type === 'number' || field.type === 'integer') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <InputNumber
          data-testid={`wf-elicit-number-${name}`}
          min={field.minimum}
          max={field.maximum}
          precision={field.type === 'integer' ? 0 : undefined}
          className="w-full"
        />
      </FormField>
    )
  }

  // ─── String formats with dedicated controls ───────────────────────────
  // The kit DatePicker stores an ISO string in form state (no Dayjs), so no
  // submit-time conversion is needed. It is date-only (no time picker), so a
  // `date-time` field selects the date and emits T00:00:00 for the time part.
  if (field.format === 'date') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <DatePicker
          data-testid={`wf-elicit-date-${name}`}
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd"
          className="w-full"
        />
      </FormField>
    )
  }
  if (field.format === 'date-time') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <DatePicker
          data-testid={`wf-elicit-datetime-${name}`}
          placeholder={`Select ${label.toLowerCase()}`}
          aria-label={label}
          valueFormat="yyyy-MM-dd'T'HH:mm:ss"
          className="w-full"
        />
      </FormField>
    )
  }
  if (field.format === 'password') {
    return (
      <FormField
        key={name}
        name={name}
        label={label}
        required={required}
        description={description}
      >
        <PasswordInput
          data-testid={`wf-elicit-password-${name}`}
          showLabel="Show"
          hideLabel="Hide"
        />
      </FormField>
    )
  }

  const inputType =
    field.format === 'email' ? 'email' : field.format === 'uri' ? 'url' : 'text'
  return (
    <FormField
      key={name}
      name={name}
      label={label}
      required={required}
      description={description}
    >
      <Input data-testid={`wf-elicit-input-${name}`} type={inputType} />
    </FormField>
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
  const schema = (elicitation.schema ?? {}) as ElicitObjectSchema
  const properties = schema.properties ?? {}
  const required = new Set(schema.required ?? [])

  const [error, setError] = useState<string | null>(null)

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

  const zodSchema = buildElicitZodSchema(properties, required)
  const form = useForm({
    resolver: zodResolver(zodSchema),
    defaultValues: initialValues,
  })

  const handleClickSubmit = async () => {
    const isValid = await form.trigger()
    if (!isValid) {
      setError('Please fix the highlighted fields')
      return
    }
    const values = form.getValues()
    // E5 parity for VIRTUAL tables: `trigger()` only checks MOUNTED
    // cells, so off-screen rows of a virtualized table escape per-cell rules.
    // Re-check each table property's full (FormList-preserved) array.
    for (const [name, field] of Object.entries(properties)) {
      if (isTableProperty(field)) {
        const err = validateTableValue(
          (values as Record<string, unknown>)[name],
          (field as any).items,
          (field as any).title || name,
        )
        if (err) {
          setError(err)
          return
        }
      }
    }
    setError(null)
    onSubmit(values as Record<string, unknown>)
  }

  return (
    <Alert
      data-testid="wf-elicit-alert"
      tone="info"
      icon={<PenLine className="size-4" />}
      title="Input required"
      description={
        <div className="mt-2 flex flex-col gap-2">
          <Text className="text-sm">{elicitation.message}</Text>
          <Form
            data-testid="wf-elicit-form"
            form={form}
            layout="vertical"
            onSubmit={() => {
              // Submit is triggered via handleClickSubmit below; this no-op
              // handler satisfies the kit Form's required onSubmit prop and
              // prevents accidental native form submission on Enter.
            }}
            disabled={submitting}
          >
            {Object.entries(properties).map(([name, field]) =>
              renderField(name, field, required.has(name), submitting),
            )}
          </Form>
          {error && (
            <Alert data-testid="wf-elicit-error-alert" tone="error" title={error} />
          )}
          <Button
            data-testid="wf-elicit-submit-btn"
            type="button"
            size="default"
            loading={submitting}
            onClick={() => {
              void handleClickSubmit()
            }}
            className="self-start"
          >
            Submit
          </Button>
        </div>
      }
    />
  )
}
