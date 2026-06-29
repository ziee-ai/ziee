/** Local validation-rule shapes (dependency-free; formerly antd's Form `Rule`).
 *  A thin async-validator rule model consumed by the workflow elicit forms. */
export interface Rule {
  required?: boolean
  message?: string
  pattern?: RegExp
  type?: 'string' | 'number' | 'array' | 'boolean' | 'object'
  min?: number
  max?: number
  validator?: (rule: unknown, value: unknown) => Promise<void>
}

/** Array-level (list) rule: validates the whole array value. */
export type ListRule = {
  validator: (rule: unknown, value: unknown[] | undefined) => Promise<void>
}

/**
 * Lightweight JSON-schema typing + antd-rule derivation shared by
 * `WorkflowElicitForm` (scalar fields) and `EditableArrayTable` (cells).
 *
 * This is deliberately a thin, dependency-free subset of JSON-schema
 * validation — just enough to block an obviously-invalid submit BEFORE
 * posting, for parity with the backend's full jsonschema enforcement
 * (which still returns 422 on anything we miss). NOT a replacement for
 * ajv; we intentionally add no new npm dependency.
 */

/** Optional, schema-valid `ui:` hints for a single property/column. */
export interface FieldUiHints {
  widget?: 'table'
  bulkToggle?: boolean
  expand?: boolean
  sortable?: boolean
  filterable?: boolean
  width?: number
}

/** Optional, schema-valid `ui:` hints for an array property. */
export interface ArrayUiHints {
  widget?: 'table'
  virtual?: boolean
  maxRows?: number
}

export interface FieldSchema {
  type?: string
  title?: string
  description?: string
  default?: unknown
  /** JSON-schema string `format` (date, date-time, email, uri, password). */
  format?: string
  // scalar constraints
  minimum?: number
  maximum?: number
  enum?: unknown[]
  /** Parallel labels for `enum` values (legacy `enumNames`). */
  enumNames?: string[]
  /** Titled single-select choices (top-level `anyOf`/`oneOf`). */
  anyOf?: TitledChoice[]
  oneOf?: TitledChoice[]
  const?: unknown
  pattern?: string
  minLength?: number
  maxLength?: number
  // array constraints
  minItems?: number
  maxItems?: number
  items?: ObjectItemsSchema
  // hints
  ui?: FieldUiHints
}

/** A titled enum choice: a JSON-schema `anyOf`/`oneOf` entry `{const, title?}`. */
export interface TitledChoice {
  const: string
  title?: string
}

/** The `items` of an `array`. For a table the elements are objects
 *  (`properties`); for a multi-select they carry `enum`/`anyOf`/`oneOf`. */
export interface ObjectItemsSchema {
  type?: string
  properties?: Record<string, FieldSchema>
  required?: string[]
  /** Multi-select choices (primitive-array items). */
  enum?: unknown[]
  anyOf?: TitledChoice[]
  oneOf?: TitledChoice[]
}

/** A property the form should render as an editable table. */
export interface ArraySchema extends FieldSchema {
  type: 'array'
  items?: ObjectItemsSchema
  ui?: ArrayUiHints & FieldUiHints
}

/** Top-level object schema carried on the elicitation. */
export interface ElicitObjectSchema {
  properties?: Record<string, FieldSchema>
  required?: string[]
}

/** True when a property should render as an editable table: either its
 *  items are objects, or it carries an explicit `ui.widget === 'table'`. */
export function isTableProperty(field: FieldSchema): field is ArraySchema {
  if (field.ui?.widget === 'table') return true
  return field.type === 'array' && field.items?.type === 'object'
}

/**
 * Derive antd `Form.Item` rules from a scalar field schema. The label is
 * only used to phrase the messages; pass the visible label.
 */
export function fieldRules(
  field: FieldSchema,
  required: boolean,
  label: string,
): Rule[] {
  const rules: Rule[] = []
  if (required) {
    rules.push({ required: true, message: `${label} is required` })
  }

  // Type / enum / pattern / numeric / string-length parity. Each rule
  // skips empty values for optional fields (antd's `required` rule owns
  // the presence check) so an untouched optional field stays valid.
  const isEmpty = (v: unknown) => v === undefined || v === null || v === ''

  if (field.enum) {
    const allowed = field.enum
    rules.push({
      validator: (_r, value) => {
        if (isEmpty(value)) return Promise.resolve()
        return allowed.includes(value)
          ? Promise.resolve()
          : Promise.reject(new Error(`${label} must be one of the allowed values`))
      },
    })
  } else if (field.type) {
    rules.push({
      validator: (_r, value) => {
        if (isEmpty(value)) return Promise.resolve()
        return matchesJsType(value, field.type!)
          ? Promise.resolve()
          : Promise.reject(new Error(`${label} must be a ${field.type}`))
      },
    })
  }

  if (field.const !== undefined) {
    // JSON-schema `const`: the value must equal the fixed constant (e.g. an
    // `approved: { const: true }` confirmation gate). Presence is owned by the
    // `required` rule; this rule rejects a present-but-wrong value (a false
    // Switch) so the form blocks submit, in parity with the backend's 422.
    const expected = field.const
    rules.push({
      validator: (_r, value) => {
        if (isEmpty(value)) return Promise.resolve()
        return value === expected
          ? Promise.resolve()
          : Promise.reject(new Error(`${label} must be ${JSON.stringify(expected)}`))
      },
    })
  }

  if (field.pattern) {
    // `new RegExp` can throw on a malformed pattern from the schema —
    // fail soft (skip the rule) rather than crash the form render.
    try {
      const re = new RegExp(field.pattern)
      rules.push({ pattern: re, message: `${label} has an invalid format` })
    } catch {
      // ignore an unparseable pattern
    }
  }

  if (field.type === 'number' || field.type === 'integer') {
    if (field.minimum !== undefined) {
      rules.push({ type: 'number', min: field.minimum, message: `${label} must be ≥ ${field.minimum}` })
    }
    if (field.maximum !== undefined) {
      rules.push({ type: 'number', max: field.maximum, message: `${label} must be ≤ ${field.maximum}` })
    }
  }

  if (field.type === 'string') {
    if (field.minLength !== undefined) {
      rules.push({ min: field.minLength, message: `${label} must be at least ${field.minLength} characters` })
    }
    if (field.maxLength !== undefined) {
      rules.push({ max: field.maxLength, message: `${label} must be at most ${field.maxLength} characters` })
    }
  }

  if (field.type === 'array') {
    if (field.minItems !== undefined) {
      rules.push({ type: 'array', min: field.minItems, message: `${label} needs at least ${field.minItems} item(s)` })
    }
    if (field.maxItems !== undefined) {
      rules.push({ type: 'array', max: field.maxItems, message: `${label} allows at most ${field.maxItems} item(s)` })
    }
  }

  return rules
}

/** Rule derivation for a table cell — same logic as `fieldRules` but the
 *  label is just the column key/title (kept short for the inline error). */
export function fieldItemRules(field: FieldSchema, required: boolean): Rule[] {
  return fieldRules(field, required, field.title || 'value')
}

/**
 * `Form.List`-level rules for an array property. A `Form.List` validator
 * receives the whole array value, so `required` / `minItems` / `maxItems`
 * are enforced here (the per-cell rules live on each cell's `Form.Item`).
 */
export function listRules(
  field: FieldSchema,
  required: boolean,
  label: string,
): ListRule[] {
  const rules: ListRule[] = []
  const min = field.minItems ?? (required ? 1 : undefined)
  if (min !== undefined) {
    rules.push({
      validator: (_r, value: unknown[] | undefined) => {
        const len = Array.isArray(value) ? value.length : 0
        return len >= min
          ? Promise.resolve()
          : Promise.reject(
              new Error(`${label} needs at least ${min} row(s)`),
            )
      },
    })
  }
  if (field.maxItems !== undefined) {
    const max = field.maxItems
    rules.push({
      validator: (_r, value: unknown[] | undefined) => {
        const len = Array.isArray(value) ? value.length : 0
        return len <= max
          ? Promise.resolve()
          : Promise.reject(new Error(`${label} allows at most ${max} row(s)`))
      },
    })
  }
  return rules
}

/**
 * Whole-array structural validation for a table property — E5 parity for the
 * VIRTUAL case. A virtualized antd `Table` only MOUNTS visible rows, so the
 * off-screen rows' per-cell `Form.Item` rules never run during
 * `validateFields()`. This walks the FULL array value (which `Form.List`
 * preserves) and checks each row's required keys + enum membership + primitive
 * type. Returns the first human-readable error, or `null` when valid. The
 * backend's full jsonschema still enforces anything this thin check misses.
 */
export function validateTableValue(
  rows: unknown,
  items: ObjectItemsSchema | undefined,
  label: string,
): string | null {
  if (!items?.properties || !Array.isArray(rows)) return null
  const required = new Set(items.required ?? [])
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]
    if (typeof row !== 'object' || row === null) continue
    const r = row as Record<string, unknown>
    for (const [key, col] of Object.entries(items.properties)) {
      const v = r[key]
      const empty = v === undefined || v === null || v === ''
      const colLabel = col.title || key
      if (required.has(key) && empty) {
        return `${label} row ${i + 1}: "${colLabel}" is required`
      }
      if (empty) continue
      if (col.enum && !col.enum.includes(v)) {
        return `${label} row ${i + 1}: "${colLabel}" must be one of the allowed values`
      }
      if (col.type && !matchesJsType(v, col.type)) {
        return `${label} row ${i + 1}: "${colLabel}" must be a ${col.type}`
      }
    }
  }
  return null
}

function matchesJsType(value: unknown, type: string): boolean {
  switch (type) {
    case 'string':
      return typeof value === 'string'
    case 'number':
      return typeof value === 'number' && !Number.isNaN(value)
    case 'integer':
      return typeof value === 'number' && Number.isInteger(value)
    case 'boolean':
      return typeof value === 'boolean'
    case 'array':
      return Array.isArray(value)
    case 'object':
      return (
        typeof value === 'object' && value !== null && !Array.isArray(value)
      )
    default:
      return true
  }
}
