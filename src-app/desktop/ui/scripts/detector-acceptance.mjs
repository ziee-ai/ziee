/**
 * DETECTOR ACCEPTANCE (desktop) — prove the audit detectors that ship in THIS
 * workspace actually fire, without importing the web workspace's 488-line
 * `DefectRepro.tsx` chat-repro apparatus (which has no desktop surface meaning).
 *
 * The desktop gallery is PAGE-focused and has no `seeded-defect-repro` surface,
 * so the geometry-DOM repro cases the web `detector-acceptance.mjs` renders do
 * not exist here. Rather than port an irrelevant 20-case chat apparatus, this
 * desktop harness proves the SAME detector code is trustworthy in two honest,
 * self-contained ways:
 *
 *   1. LINT detectors (source-visible `[L]` classes — C11 icon-action,
 *      J8 native-scroll): run the REAL lint against the copied
 *      `src/dev/gallery/__detector_fixtures__/` and assert each FIRES. Fully
 *      self-contained; needs no dev server. This is identical in spirit to the
 *      web harness's lint rows.
 *
 *   2. GEOMETRY detector (the in-page `[G]` detector): the desktop copy of
 *      `gallery-geometry-audit.mjs` is a byte-faithful copy of the web version,
 *      whose OWN `detector-acceptance.mjs` already renders + validates every
 *      geometry repro against it. So the geometry detector is proven-correct
 *      by IDENTITY: assert the desktop copy's detector core is byte-identical to
 *      the web source (a drift guard). If the two ever diverge, this fails and
 *      forces a real re-validation.
 *
 *   node scripts/detector-acceptance.mjs [--json=OUT]
 *
 * No gallery dev server required (unlike the web harness).
 */
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const WEB_SCRIPTS = path.resolve(UI_DIR, '../../ui/scripts')

const arg = (n, d) =>
  (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`)
    .split('=')
    .slice(1)
    .join('=')
const JSON_OUT = arg('json', '')

const FIXTURE_DIR = 'src/dev/gallery/__detector_fixtures__'

// ───────────────────────────────────────────────────────────────────────────
// (1) LINT detector cases — run against the copied source fixtures.
// ───────────────────────────────────────────────────────────────────────────
const LINT_CASES = [
  { miss: '#10b', cls: 'C11', lint: 'lint-icon-action.mjs', extra: [], desc: 'open-in-new-tab renders the wrong glyph' },
  { miss: '#17', cls: 'J8', lint: 'lint-native-scroll.mjs', extra: ['--gate'], desc: 'raw native scroll instead of DivScrollY' },
]

function runLint(script, extra = []) {
  // Fail loudly if the detector itself is missing — otherwise `node` exits
  // non-zero for a MISSING script and we would miscount that as the detector
  // "firing" (a false pass that hides a dropped detector).
  const scriptPath = path.join(UI_DIR, 'scripts', script)
  if (!fs.existsSync(scriptPath)) {
    return { fired: false, exit: -1, out: `MISSING detector script: scripts/${script}` }
  }
  const res = spawnSync(
    'node',
    [path.join('scripts', script), `--root=${FIXTURE_DIR}`, ...extra],
    { cwd: UI_DIR, encoding: 'utf8' },
  )
  const out = `${res.stdout || ''}${res.stderr || ''}`
  // A lint FIRES when it exits non-zero OR prints a finding line for the fixture.
  const fired = res.status !== 0 || /__detector_fixtures__/.test(out)
  return { fired, exit: res.status, out }
}

// ───────────────────────────────────────────────────────────────────────────
// (2) GEOMETRY detector byte-identity drift guard.
// ───────────────────────────────────────────────────────────────────────────
function geometryIdentity() {
  const local = path.join(__dirname, 'gallery-geometry-audit.mjs')
  const web = path.join(WEB_SCRIPTS, 'gallery-geometry-audit.mjs')
  if (!fs.existsSync(web)) return { ok: false, detail: 'web source geometry script missing' }
  const same =
    fs.readFileSync(local, 'utf8') === fs.readFileSync(web, 'utf8')
  return {
    ok: same,
    detail: same
      ? 'byte-identical to web source (validated by web detector-acceptance)'
      : 'DRIFTED from web source — re-run web detector-acceptance and reconcile',
  }
}

function main() {
  console.log('detector-acceptance (desktop): proving the shipped detectors fire…\n')
  const rows = []

  for (const c of LINT_CASES) {
    const r = runLint(c.lint, c.extra)
    rows.push({ ...c, kind: 'lint', fired: r.fired, detail: `exit ${r.exit}` })
  }

  const geo = geometryIdentity()
  rows.push({
    miss: '#1-21', cls: 'G*', kind: 'geometry-identity', fired: geo.ok, detail: geo.detail,
  })

  const w = (s, n) => String(s).padEnd(n)
  console.log(w('miss', 8) + w('class', 7) + w('kind', 18) + w('status', 9) + 'detail')
  console.log('─'.repeat(96))
  let failed = 0
  for (const r of rows) {
    const status = r.fired ? 'OK ✓' : 'FAIL ✗'
    if (!r.fired) failed++
    console.log(w(r.miss, 8) + w(r.cls, 7) + w(r.kind, 18) + w(status, 9) + `${r.desc || ''} — ${r.detail}`)
  }
  console.log('─'.repeat(96))

  if (JSON_OUT) {
    fs.writeFileSync(path.resolve(UI_DIR, JSON_OUT), JSON.stringify(rows, null, 2))
    console.log(`  → ${JSON_OUT}`)
  }

  if (failed) {
    console.error(`\n❌ DETECTOR-ACCEPTANCE FAILED — ${failed} check(s) did not pass.`)
    process.exit(1)
  }
  console.log('\n✅ DETECTOR-ACCEPTANCE PASSED — lint detectors fire + geometry detector matches validated web source.')
}

main()
