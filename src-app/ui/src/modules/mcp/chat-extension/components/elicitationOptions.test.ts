import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  allowsOther,
  buildFormSchema,
  finalizeValues,
  getRichOptions,
  isChoiceField,
  isMultiChoiceField,
  isOtherSelected,
  orderRecommendedFirst,
  otherFieldError,
  OTHER_SENTINEL,
  type FieldSchema,
} from './elicitationOptions.ts'

// ── TEST-5: getRichOptions across all four SEP-1330 shapes + rich fields ─────

test('getRichOptions: legacy enum carries names/descriptions/previews + recommended', () => {
  const fs: FieldSchema = {
    type: 'string',
    enum: ['oauth', 'password', 'ldap'],
    enumNames: ['OAuth 2.0', 'Password', 'LDAP'],
    enumDescriptions: ['Delegate to Google', 'Local creds', null],
    enumPreviews: ['flow: redirect', null, null],
    'x-ziee-recommended': 'oauth',
  }
  const opts = getRichOptions(fs)
  assert.deepEqual(
    opts.map(o => o.value),
    ['oauth', 'password', 'ldap'],
  )
  assert.equal(opts[0].label, 'OAuth 2.0')
  assert.equal(opts[0].description, 'Delegate to Google')
  assert.equal(opts[0].preview, 'flow: redirect')
  assert.equal(opts[0].recommended, true)
  assert.equal(opts[1].recommended, false)
  // null description/preview coerce to undefined (never the string "null").
  assert.equal(opts[2].description, undefined)
})

test('getRichOptions: titled oneOf/anyOf carries per-entry description/preview/recommended', () => {
  const fs: FieldSchema = {
    type: 'string',
    oneOf: [
      { const: 'a', title: 'Alpha', description: 'first', recommended: true },
      { const: 'b', title: 'Beta', preview: 'x=1' },
    ],
  }
  const opts = getRichOptions(fs)
  assert.equal(opts[0].label, 'Alpha')
  assert.equal(opts[0].description, 'first')
  assert.equal(opts[0].recommended, true)
  assert.equal(opts[1].preview, 'x=1')
})

test('getRichOptions: multi-select (items.enum) shape', () => {
  const fs: FieldSchema = {
    type: 'array',
    items: {
      enum: ['red', 'green'],
      enumDescriptions: ['warm', 'cool'],
    },
    'x-ziee-recommended': 'green',
  }
  const opts = getRichOptions(fs)
  assert.deepEqual(opts.map(o => o.value), ['red', 'green'])
  assert.equal(opts[1].description, 'cool')
  assert.equal(opts[1].recommended, true)
})

test('getRichOptions: non-choice field yields no options', () => {
  assert.deepEqual(getRichOptions({ type: 'string', format: 'email' }), [])
  assert.deepEqual(getRichOptions({ type: 'number' }), [])
})

// ── TEST-6: buildFormSchema preserves validation exactly ─────────────────────

test('buildFormSchema: required + email + pattern + number bounds + multiselect min', () => {
  const props: Record<string, FieldSchema> = {
    email: { type: 'string', format: 'email', title: 'Email' },
    code: { type: 'string', pattern: '^[A-Z]{3}$', title: 'Code' },
    age: { type: 'number', minimum: 18, maximum: 99, title: 'Age' },
    colors: { type: 'array', items: { enum: ['r', 'g', 'b'] }, minItems: 2, title: 'Colors' },
  }
  const schema = buildFormSchema(
    props,
    new Set(['email', 'code', 'age', 'colors']),
  )

  // Required: nothing provided → fails on every field.
  assert.equal(schema.safeParse({}).success, false)

  // Invalid email / bad pattern / out-of-range / too-few items all fail.
  assert.equal(
    schema.safeParse({ email: 'nope', code: 'ABC', age: 20, colors: ['r', 'g'] }).success,
    false,
    'invalid email must fail',
  )
  assert.equal(
    schema.safeParse({ email: 'a@b.io', code: 'abc', age: 20, colors: ['r', 'g'] }).success,
    false,
    'lowercase code violates pattern',
  )
  assert.equal(
    schema.safeParse({ email: 'a@b.io', code: 'ABC', age: 5, colors: ['r', 'g'] }).success,
    false,
    'age below minimum',
  )
  assert.equal(
    schema.safeParse({ email: 'a@b.io', code: 'ABC', age: 20, colors: ['r'] }).success,
    false,
    'too few colors',
  )

  // All valid → passes.
  assert.equal(
    schema.safeParse({ email: 'a@b.io', code: 'ABC', age: 20, colors: ['r', 'g'] }).success,
    true,
  )
})

test('buildFormSchema: free text (incl. the Other answer) satisfies a choice field', () => {
  // A choice field is `z.string().min(1)` — NOT enum-restricted — so an
  // arbitrary Other free-text value validates without any zod change.
  const schema = buildFormSchema(
    { choice: { type: 'string', enum: ['a', 'b'], title: 'Choice' } },
    new Set(['choice']),
  )
  assert.equal(schema.safeParse({ choice: 'something custom' }).success, true)
  assert.equal(schema.safeParse({ choice: '' }).success, false)
})

// ── TEST-7: orderRecommendedFirst ────────────────────────────────────────────

test('orderRecommendedFirst: recommended moves to index 0, rest stable', () => {
  const opts = [
    { value: 'a', label: 'A' },
    { value: 'b', label: 'B', recommended: true },
    { value: 'c', label: 'C' },
  ]
  const ordered = orderRecommendedFirst(opts)
  assert.deepEqual(ordered.map(o => o.value), ['b', 'a', 'c'])
})

test('orderRecommendedFirst: no recommended → unchanged; already-first → unchanged', () => {
  const none = [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]
  assert.deepEqual(orderRecommendedFirst(none), none)
  const firstRec = [{ value: 'a', label: 'A', recommended: true }, { value: 'b', label: 'B' }]
  assert.deepEqual(orderRecommendedFirst(firstRec).map(o => o.value), ['a', 'b'])
})

// ── TEST-8: choice detection + Other affordance + sentinel ───────────────────

test('isChoiceField / isMultiChoiceField detect all choice shapes', () => {
  assert.equal(isChoiceField({ type: 'string', enum: ['a'] }), true)
  assert.equal(isChoiceField({ type: 'string', oneOf: [{ const: 'a' }] }), true)
  assert.equal(isMultiChoiceField({ type: 'array', items: { enum: ['a'] } }), true)
  assert.equal(isChoiceField({ type: 'string', format: 'email' }), false)
  assert.equal(isChoiceField({ type: 'boolean' }), false)
})

test('allowsOther: default on for choices, opt-out via x-ziee-allow-other:false', () => {
  assert.equal(allowsOther({ type: 'string', enum: ['a'] }), true)
  assert.equal(
    allowsOther({ type: 'string', enum: ['a'], 'x-ziee-allow-other': false }),
    false,
  )
  // Non-choice fields never offer Other.
  assert.equal(allowsOther({ type: 'string', format: 'email' }), false)
})

test('OTHER_SENTINEL is distinct from realistic option values', () => {
  for (const v of ['other', 'Other', 'oauth', 'yes', 'no', '', 'null', '__other__']) {
    assert.notEqual(v, OTHER_SENTINEL)
  }
})

// ── Other-escape ⇄ response-envelope helpers (single + multi) ────────────────

const single: FieldSchema = { type: 'string', enum: ['a', 'b'] }
const multi: FieldSchema = { type: 'array', items: { enum: ['a', 'b'] } }

test('isOtherSelected detects the sentinel for single and multi', () => {
  assert.equal(isOtherSelected(single, OTHER_SENTINEL), true)
  assert.equal(isOtherSelected(single, 'a'), false)
  assert.equal(isOtherSelected(multi, ['a', OTHER_SENTINEL]), true)
  assert.equal(isOtherSelected(multi, ['a', 'b']), false)
})

test('otherFieldError requires the free text only when Other is selected', () => {
  assert.equal(otherFieldError(single, OTHER_SENTINEL, ''), 'Enter a value for “Other”.')
  assert.equal(otherFieldError(single, OTHER_SENTINEL, '  '), 'Enter a value for “Other”.')
  assert.equal(otherFieldError(single, OTHER_SENTINEL, 'custom'), null)
  assert.equal(otherFieldError(single, 'a', ''), null) // not Other → no error
  assert.equal(otherFieldError(multi, ['a', OTHER_SENTINEL], ''), 'Enter a value for “Other”.')
  assert.equal(otherFieldError(multi, ['a', OTHER_SENTINEL], 'z'), null)
})

test('finalizeValues replaces the single-select sentinel with the free text', () => {
  const out = finalizeValues({ c: single }, { c: OTHER_SENTINEL }, { c: 'custom' })
  assert.equal(out.c, 'custom')
})

test('finalizeValues strips the multi sentinel and appends the free text', () => {
  const out = finalizeValues({ c: multi }, { c: ['a', OTHER_SENTINEL] }, { c: 'zed' })
  assert.deepEqual(out.c, ['a', 'zed'])
  // Empty Other text → sentinel dropped, nothing appended.
  const empty = finalizeValues({ c: multi }, { c: ['a', OTHER_SENTINEL] }, { c: '  ' })
  assert.deepEqual(empty.c, ['a'])
})

test('finalizeValues leaves non-Other answers + Other-disabled fields untouched', () => {
  // A normal answer passes through.
  assert.equal(finalizeValues({ c: single }, { c: 'a' }, {}).c, 'a')
  // A field that does NOT offer Other is never rewritten, even if its value
  // literally equals the sentinel (the value-space-collision guard).
  const noOther: FieldSchema = { type: 'string', enum: ['a'], 'x-ziee-allow-other': false }
  assert.equal(
    finalizeValues({ c: noOther }, { c: OTHER_SENTINEL }, { c: 'x' }).c,
    OTHER_SENTINEL,
  )
})
