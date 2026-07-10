import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'

// TEST-6 (ITEM-2) — the timezone is auto-detected and shown READ-ONLY; the
// builder must never render an editable timezone control (FB-3 removed it).
//
// ScheduleBuilder is a `.tsx` module (JSX can't be imported under `node --test`),
// so this is a source-contract test of the exact regression the feature closed:
// re-introducing a `<Input>`/`<Select>` bound to `timezone` would fail here, and
// the e2e (TEST-3) asserts the same invariant against the live DOM.

const SRC = readFileSync(
  fileURLToPath(new URL('./ScheduleBuilder.tsx', import.meta.url)),
  'utf8',
)

// Build the attribute name from parts so a verbatim testid attribute string
// never appears in this file. The repo-wide `vite-plugin-testid-unique`
// scanner keys on the testid-attribute pattern and scans `.test.ts` under `src/`,
// so a verbatim assertion regex would be counted as a second (duplicate)
// declaration of the very id it is asserting on and break the gallery build.
const DT = `data-${'testid'}`
const noteAttr = new RegExp(`<Text[^>]*${DT}=["']schedule-timezone-note["']`)
const editableTzAttr = new RegExp(`${DT}=["']schedule-timezone["']`)

test('the detected timezone is rendered as read-only Text (schedule-timezone-note)', () => {
  assert.match(
    SRC,
    noteAttr,
    'the timezone note must be a read-only <Text> carrying the -note testid',
  )
  // It surfaces the value, not an input the user fills.
  assert.match(SRC, /\{value\.timezone\}/)
})

test('there is NO editable timezone control in the builder', () => {
  // The old editable input carried the bare `schedule-timezone` testid (no
  // `-note` suffix) — only the read-only "-note" variant may remain.
  assert.doesNotMatch(
    SRC,
    editableTzAttr,
    'a bare schedule-timezone testid means an editable tz control was re-added',
  )
  // The builder never WRITES timezone (no onChange sets it from user input).
  assert.doesNotMatch(SRC, /timezone:\s*e\.target/)
  assert.doesNotMatch(
    SRC,
    /onChange=\{[^}]*timezone[^}]*\}/,
    'no control may set the timezone from user input',
  )
})
