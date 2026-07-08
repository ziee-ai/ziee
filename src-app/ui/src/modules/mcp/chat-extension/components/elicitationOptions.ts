import { z } from 'zod'
import { EMAIL_RE } from '@/lib/validation'

/**
 * Shared, pure schema helpers for the elicitation form + the rich `ask_user`
 * decision UX. Extracted from `ElicitationFormContent.tsx` so both the legacy
 * (external-MCP, flat) renderer and the rich `AskUserWizardContent` share ONE
 * source of truth for option extraction + zod validation, and so the pure logic
 * is unit-testable.
 */

/** Sentinel selected value that means "the user chose the free-text Other option". */
export const OTHER_SENTINEL = '__ziee_other__'

/** Root marker the backend stamps on the ziee-internal `ask_user` schema. */
export const ASK_USER_MARKER = 'x-ziee-askuser'

/** A titled option entry (the `oneOf`/`anyOf` form). */
interface OptionEntry {
  const: string
  title?: string
  description?: string
  preview?: string
  recommended?: boolean
}

export interface FieldSchema {
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
  /** ziee rich convention: per-option trade-off text, index-aligned with `enum`. */
  enumDescriptions?: (string | null)[]
  /** ziee rich convention: per-option monospace preview, index-aligned with `enum`. */
  enumPreviews?: (string | null)[]
  anyOf?: OptionEntry[]
  oneOf?: OptionEntry[]
  items?: {
    type?: string
    enum?: string[]
    enumNames?: string[]
    enumDescriptions?: (string | null)[]
    enumPreviews?: (string | null)[]
    anyOf?: OptionEntry[]
    oneOf?: OptionEntry[]
  }
  /** ziee rich convention: the enum VALUE to render first + badge "Recommended". */
  'x-ziee-recommended'?: string
  /** ziee rich convention: default true — set false to suppress the Other escape. */
  'x-ziee-allow-other'?: boolean
}

/** A choice option enriched with the rich decision-UX fields. */
export interface RichOption {
  value: string
  label: string
  description?: string
  preview?: string
  recommended?: boolean
}

// ─── Field-shape predicates ──────────────────────────────────────────────────

export function isMultiChoiceField(fs: FieldSchema): boolean {
  return (
    fs.type === 'array' &&
    !!(fs.items?.enum || fs.items?.anyOf || fs.items?.oneOf)
  )
}

export function isSingleChoiceField(fs: FieldSchema): boolean {
  return fs.type === 'string' && !!(fs.enum || fs.anyOf || fs.oneOf)
}

export function isChoiceField(fs: FieldSchema): boolean {
  return isMultiChoiceField(fs) || isSingleChoiceField(fs)
}

/** Whether a choice field offers the always-available Other free-text escape. */
export function allowsOther(fs: FieldSchema): boolean {
  return isChoiceField(fs) && fs['x-ziee-allow-other'] !== false
}

// ─── Option extraction ───────────────────────────────────────────────────────

function fromEntries(entries: OptionEntry[]): RichOption[] {
  return entries.map(o => ({
    value: o.const,
    label: o.title ?? o.const,
    description: o.description,
    preview: o.preview ?? undefined,
    recommended: o.recommended === true,
  }))
}

function fromEnum(
  values: string[],
  names: string[] | undefined,
  descriptions: (string | null)[] | undefined,
  previews: (string | null)[] | undefined,
  recommendedValue: string | undefined,
): RichOption[] {
  return values.map((v, i) => ({
    value: v,
    label: names?.[i] ?? v,
    description: descriptions?.[i] ?? undefined,
    preview: previews?.[i] ?? undefined,
    recommended: recommendedValue != null && recommendedValue === v,
  }))
}

/**
 * Extract the choice options (with rich metadata) for a field, across all four
 * SEP-1330 shapes: titled single (`anyOf`/`oneOf`), legacy single (`enum`),
 * titled multi (`items.anyOf`/`oneOf`), legacy multi (`items.enum`).
 */
export function getRichOptions(fs: FieldSchema): RichOption[] {
  if (fs.type === 'string' && (fs.anyOf || fs.oneOf)) {
    return fromEntries((fs.anyOf ?? fs.oneOf)!)
  }
  if (fs.type === 'string' && fs.enum) {
    return fromEnum(
      fs.enum,
      fs.enumNames,
      fs.enumDescriptions,
      fs.enumPreviews,
      fs['x-ziee-recommended'],
    )
  }
  if (fs.type === 'array' && (fs.items?.anyOf || fs.items?.oneOf)) {
    return fromEntries((fs.items.anyOf ?? fs.items.oneOf)!)
  }
  if (fs.type === 'array' && fs.items?.enum) {
    return fromEnum(
      fs.items.enum,
      fs.items.enumNames,
      fs.items.enumDescriptions,
      fs.items.enumPreviews,
      fs['x-ziee-recommended'],
    )
  }
  return []
}

/** Plain `{value,label}` options for the legacy (non-rich) Select/MultiSelect. */
export function getOptions(fs: FieldSchema): { value: string; label: string }[] {
  return getRichOptions(fs).map(({ value, label }) => ({ value, label }))
}

/**
 * Move the first option flagged `recommended` to index 0 (stable for the rest).
 * No recommended option → order unchanged.
 */
export function orderRecommendedFirst(options: RichOption[]): RichOption[] {
  const idx = options.findIndex(o => o.recommended)
  if (idx <= 0) return options
  const copy = options.slice()
  const [rec] = copy.splice(idx, 1)
  copy.unshift(rec)
  return copy
}

// ─── zod validation (unchanged from the original renderer) ───────────────────

/** Build a zod schema for a single field. */
export function buildFieldZodSchema(
  fieldSchema: FieldSchema,
  required: boolean,
): z.ZodTypeAny {
  const label = fieldSchema.title ?? 'This field'
  const isMultiSelect = isMultiChoiceField(fieldSchema)
  const isSelectField = isMultiSelect || isSingleChoiceField(fieldSchema)

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
export function buildFormSchema(
  properties: Record<string, FieldSchema>,
  requiredFields: Set<string>,
): z.ZodObject<Record<string, z.ZodTypeAny>> {
  const shape: Record<string, z.ZodTypeAny> = {}
  for (const [name, fieldSchema] of Object.entries(properties)) {
    shape[name] = buildFieldZodSchema(fieldSchema, requiredFields.has(name))
  }
  return z.object(shape)
}
