import { PenLine } from 'lucide-react'
import {
  Alert,
  Button,
  Form,
  FormField,
  Input,
  InputNumber,
  Select,
  Switch,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { useState } from 'react'
import type { SSEElicitationRequiredData } from '@/api-client/types'
import { EditableArrayTable } from './EditableArrayTable'
import {
  type ElicitObjectSchema,
  type FieldSchema,
  isTableProperty,
  validateTableValue,
} from './workflowElicitSchema'

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
  } else if (isTableProperty(field)) {
    // Array-of-objects: build per-column zod shape so per-cell errors surface inline.
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
    if (required && f.minItems !== undefined && f.minItems > 0) {
      arr = arr.min(f.minItems)
    } else if (f.minItems !== undefined && f.minItems > 0) {
      arr = arr.min(f.minItems)
    }
    if (f.maxItems !== undefined) arr = arr.max(f.maxItems)
    // Arrays are always present (even if empty); mark optional only when not required.
    return required ? arr : arr.optional()
  } else {
    // default: string
    schema = z.string()
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
        <Switch />
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
          min={field.minimum}
          max={field.maximum}
          precision={field.type === 'integer' ? 0 : undefined}
          className="w-full"
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
      description={description}
    >
      <Input />
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
      tone="info"
      icon={<PenLine className="size-4" />}
      title="Input required"
      description={
        <div className="mt-2">
          <Text className="text-sm">{elicitation.message}</Text>
          <Form
            form={form}
            layout="vertical"
            className="mt-3"
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
            <Alert tone="error" title={error} className="!mb-2 mt-2" />
          )}
          <Button
            type="button"
            size="sm"
            loading={submitting}
            onClick={() => {
              void handleClickSubmit()
            }}
            className="mt-2"
          >
            Submit
          </Button>
        </div>
      }
    />
  )
}
