import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

// TEST-62 (split-chat ITEM-42): COVERAGE_GAPS.md is a DURABLE, committed tracking
// doc — the 5-agent coverage sweep's output, kept beside the 14-split-chat specs so
// it survives the `.lifecycle/` merge-strip (DEC-56). This test locks its shape in
// so the doc can't silently rot into an empty stub: it must keep its required
// sections, its ≥5 candidate-bug rows, and its cross-references to the items that
// close the reported gaps. Mirrors the doc-structural pattern of `galleryCoverage.test.ts`.

const DOC = readFileSync(
  fileURLToPath(
    new URL(
      '../../../../../tests/e2e/14-split-chat/COVERAGE_GAPS.md',
      import.meta.url,
    ),
  ),
  'utf8',
)

for (const heading of [
  '# split-chat-multipane — test coverage gaps',
  '## Two meta-findings',
  '## Candidate bugs',
  '## Addressed this round',
  '## Deferred, prioritized',
  '## Cleared hypotheses',
]) {
  test(`COVERAGE_GAPS.md has the "${heading}" section`, () => {
    assert.ok(
      DOC.includes(heading),
      `missing required section: ${heading}`,
    )
  })
}

test('COVERAGE_GAPS.md lists at least 5 candidate-bug rows (B1..Bn)', () => {
  const bugRows = [...DOC.matchAll(/^\| B\d+ \|/gm)]
  assert.ok(
    bugRows.length >= 5,
    `expected ≥5 candidate-bug rows, found ${bugRows.length}`,
  )
})

test('COVERAGE_GAPS.md cross-references the items that close the reported gaps', () => {
  // ITEM-40 (composerOwnership unit) + ITEM-41 (per-pane file e2e) are the round-2
  // deliverables that address the file-isolation gap FB-6 reported; the doc must
  // point at them so the "addressed vs deferred" split stays honest.
  assert.ok(DOC.includes('ITEM-40'), 'must reference ITEM-40')
  assert.ok(DOC.includes('ITEM-41'), 'must reference ITEM-41')
  assert.ok(DOC.includes('ITEM-42'), 'must reference ITEM-42 (this doc)')
})

test('COVERAGE_GAPS.md records the two meta-findings (phantom coverage + untested FIX_ROUND bugs)', () => {
  assert.ok(/phantom coverage/i.test(DOC), 'must name the phantom-coverage finding')
  assert.ok(/FIX_ROUND/i.test(DOC), 'must name the untested-FIX_ROUND-bug finding')
})
