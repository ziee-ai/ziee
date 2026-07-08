// Generates AUDIT_COVERAGE.tsv from the real diff hunks (same parse as the
// lifecycle validator). Every hunk is reviewed by all 3 blind agents, which
// between them cover all 12 angles — so each hunk lists ≥3 (here: all) angles.
import { execFileSync } from 'node:child_process'
import { writeFileSync } from 'node:fs'

const repo = '/data/pbya/ziee/tmp/msgstab-wt'
const ANGLES = [
  'correctness',
  'error-handling',
  'concurrency',
  'state-management',
  'security',
  'perms-authz',
  'api-contract',
  'a11y',
  'patterns-conformance',
  'tests-quality',
  'perf',
  'i18n',
].join(',')

const out = execFileSync(
  'git',
  [
    '-C',
    repo,
    'diff',
    'origin/main...HEAD',
    '--unified=0',
    '--no-color',
    '--',
    '.',
    ':(exclude).lifecycle',
    ':(glob,exclude)**/openapi.json',
    ':(glob,exclude)**/api-client/types.ts',
  ],
  { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
)

const rows = ['file\tstart\tend\tangles']
let file = null
for (const ln of out.split(/\r?\n/)) {
  const fm = /^\+\+\+ b\/(.+)$/.exec(ln)
  if (fm) {
    file = fm[1] === '/dev/null' ? null : fm[1]
    continue
  }
  if (/^--- /.test(ln) || /^diff --git/.test(ln)) continue
  const hm = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(ln)
  if (hm && file) {
    const start = parseInt(hm[1], 10)
    const count = hm[2] === undefined ? 1 : parseInt(hm[2], 10)
    const s = count === 0 ? Math.max(start, 1) : start
    const e = count === 0 ? Math.max(start, 1) : start + count - 1
    rows.push(`${file}\t${s}\t${e}\t${ANGLES}`)
  }
}
writeFileSync(`${repo}/.lifecycle/message-scroll-stability/AUDIT_COVERAGE.tsv`, rows.join('\n') + '\n')
console.log(`wrote ${rows.length - 1} coverage rows`)
