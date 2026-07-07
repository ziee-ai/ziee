/**
 * DETECTOR ACCEPTANCE — prove every detector actually FIRES on its known-bad
 * instance. The "trust the instrument" gate.
 *
 * ROOT CAUSE this closes: a geometry/runtime/lint detector that returns 0
 * findings is indistinguishable from a detector that is silently broken or
 * mis-scoped — and several taxonomy `[G]` classes were reported "0 findings
 * app-wide" for exactly that reason (the bad STATE was never rendered, or the
 * detector was a no-op). A detector you never watched FIRE is not trustworthy.
 *
 * This harness renders every geometrically/runtime-expressible taxonomy miss
 * (`docs/DEFECT_TAXONOMY.md`, user misses #1-21) as an intentionally-defective,
 * individually-testid'd cell on the `seeded-defect-repro` gallery surface
 * (`src/dev/gallery/DefectRepro.tsx`), runs the REAL detector code against it
 * (imported from `gallery-geometry-audit.mjs` — no copy, no drift), and ASSERTS
 * each detector reports ≥1 finding of its expected class on its expected cell.
 * RED (exit 1) if any detector fails to fire.
 *
 * Source-lint classes (`[L]`: C11 icon-action, J8 native-scroll) can't be seen
 * by DOM geometry — their bad instance lives in SOURCE — so their fixtures live
 * in `src/dev/gallery/__detector_fixtures__/` and the harness runs the lint with
 * `--root` at that dir, expecting a violation.
 *
 * Pure-`[V]` classes (J5 density-variant, C13 valueless-decoration, M1
 * affordance-absent) have NO automatable detector — only a vision rubric line —
 * so they are listed as VISION (informational), never gating. This is the honest
 * boundary of the machine layer, not a silenced detector.
 *
 *   node scripts/detector-acceptance.mjs [--url=BASE] [--json=OUT]
 *
 * Requires the gallery dev server running (GALLERY_PORT, default 1420).
 */
import { chromium } from '@playwright/test'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  inPageGeometry,
  CONTEXT_TESTIDS,
  ACTION_SIDE_TOKENS,
  CLASS_SEVERITY,
} from './gallery-geometry-audit.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')

const arg = (n, d) =>
  (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`)
    .split('=')
    .slice(1)
    .join('=')
const PORT = process.env.GALLERY_PORT || '1420'
const BASE = arg('url', `http://localhost:${PORT}/dev-gallery.html`)
const JSON_OUT = arg('json', '')
const REPRO_SURFACE = 'seeded-defect-repro'
const FIXTURE_DIR = 'src/dev/gallery/__detector_fixtures__'

// ───────────────────────────────────────────────────────────────────────────
// The acceptance table: one row per taxonomy miss. `kind`:
//   geometry — assert the in-page geometry detector reports `class` on a cell
//              whose selector/detail contains `where` (its repro testid).
//   lint     — run `lint` with --root at the fixture dir; expect a violation.
//   vision   — no automatable detector (a [V] rubric line); informational only.
// ───────────────────────────────────────────────────────────────────────────
const ACCEPTANCE = [
  { miss: '#1', cls: 'A1', kind: 'geometry', where: 'repro-a1', desc: 'zero-gap adjacency (Disconnected/Connect)' },
  { miss: '#2/3', cls: 'B1', kind: 'geometry', where: 'repro-b1', desc: 'premature wrap (fits on one row)' },
  { miss: '#4', cls: 'C1', kind: 'geometry', where: 'repro-c1', desc: 'status badge before its label' },
  { miss: '#5', cls: 'G7', kind: 'geometry', where: 'repro-g7', desc: 'focus ring clipped by overflow ancestor' },
  { miss: '#6', cls: 'C7', kind: 'geometry', where: 'repro-usr', desc: 'user vs assistant indistinguishable' },
  { miss: '#7a', cls: 'C9', kind: 'geometry', where: 'repro-c9', desc: 'icon/label split across lines' },
  { miss: '#7b', cls: 'C10', kind: 'geometry', where: 'repro-c10', desc: 'icon disproportionate to text' },
  { miss: '#8', cls: 'K1', kind: 'geometry', where: 'conversation-title', desc: 'persistent context inside scroll' },
  { miss: '#9b', cls: 'I5', kind: 'geometry', where: 'repro-i5', desc: 'horizontal strip scrolls vertically' },
  { miss: '#9c', cls: 'A8', kind: 'geometry', where: 'repro-a8', desc: 'strip child off the row center' },
  { miss: '#10a', cls: 'J6', kind: 'geometry', where: 'repro-j6', desc: 'mixed button variants in a peer group' },
  { miss: '#11a', cls: 'L1', kind: 'geometry', where: '', desc: 'math fell back to raw TeX' },
  { miss: '#11b', cls: 'L2', kind: 'geometry', where: '', desc: 'mermaid fell back to raw source' },
  { miss: '#11c', cls: 'L3', kind: 'geometry', where: 'repro-l3', desc: 'syntax highlighting absent' },
  { miss: '#12', cls: 'J7', kind: 'geometry', where: 'repro-j7', desc: 'same action on opposite sides' },
  { miss: '#13a', cls: 'C12', kind: 'geometry', where: 'repro-c12', desc: 'bare placeholder avatar circle' },
  { miss: '#15', cls: 'A9', kind: 'geometry', where: 'repro-a9', desc: 'peer chips, unequal icon sizes' },
  { miss: '#16', cls: 'A10', kind: 'geometry', where: 'repro-a10', desc: 'inline edit input collapsed to zero' },
  { miss: '#18', cls: 'A11', kind: 'geometry', where: 'repro-a11', desc: 'card border clipped by overflow' },
  { miss: '#21c', cls: 'A12', kind: 'geometry', where: 'repro-a12', desc: 'cramped double-border outline button' },
  { miss: '#21a', cls: 'G9', kind: 'geometry', where: 'repro-g9', desc: 'hover-only controls reserve no space' },
  { miss: '#20', cls: 'H7', kind: 'geometry', where: 'repro-h7', desc: 'empty model select renders nothing' },
  { miss: '#10b', cls: 'C11', kind: 'lint', lint: 'lint-icon-action.mjs', desc: 'open-in-new-tab renders the wrong glyph' },
  { miss: '#17', cls: 'J8', kind: 'lint', lint: 'lint-native-scroll.mjs', extra: ['--gate'], desc: 'raw native scroll instead of DivScrollY' },
  { miss: '#9a', cls: 'J5', kind: 'vision', desc: 'button-look tabs in a dense side panel (density-variant)' },
  { miss: '#13b', cls: 'C13', kind: 'vision', desc: 'valueless decoration (avatar with no value)' },
  { miss: '#14', cls: 'M1', kind: 'vision', desc: 'affordance absent (mermaid/html need source/render toggle)' },
]

async function collectGeometryFindings() {
  const browser = await chromium.launch({
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'],
  })
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } })
  const url = `${BASE}?surface=${REPRO_SURFACE}&state=loaded&theme=light`
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 })
  // let the seeded frame + fixtures settle (the repro renders synchronously but
  // the lazy chunk + fonts need a beat).
  await page.waitForSelector('[data-testid="defect-repro-root"]', { timeout: 15_000 })
  await page.waitForTimeout(1200)
  const { findings, actionSides } = await page.evaluate(inPageGeometry, {
    classesArg: [],
    contextTestids: CONTEXT_TESTIDS,
    actionTokens: ACTION_SIDE_TOKENS,
    preview: false,
  })
  await browser.close()

  // J7 is aggregated in Node (cross-container). Replicate the audit's rule so a
  // token appearing on BOTH sides yields a J7 finding on the repro surface.
  const byAction = {}
  for (const a of actionSides) (byAction[a.action] ??= []).push(a)
  for (const [action, list] of Object.entries(byAction)) {
    const sides = new Set(list.map(a => a.side))
    if (sides.size <= 1) continue
    const counts = {
      left: list.filter(a => a.side === 'left').length,
      right: list.filter(a => a.side === 'right').length,
    }
    const majority = counts.left >= counts.right ? 'left' : 'right'
    for (const a of list.filter(a => a.side !== majority))
      findings.push({
        cls: 'J7',
        selector: a.selector,
        detail: `"${action}" on the ${a.side} but ${majority} in the majority`,
        severity: 'MEDIUM',
      })
  }
  return findings.map(f => ({ ...f, severity: f.severity || CLASS_SEVERITY[f.cls] || 'LOW' }))
}

function runLint(script, extra = []) {
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

async function main() {
  console.log(`detector-acceptance: rendering ${REPRO_SURFACE} + fixtures, proving each detector FIRES…\n`)
  const geoFindings = await collectGeometryFindings()
  const firedClasses = new Set(geoFindings.map(f => f.cls))

  const rows = []
  for (const item of ACCEPTANCE) {
    if (item.kind === 'geometry') {
      const hits = geoFindings.filter(
        f =>
          f.cls === item.cls &&
          (!item.where ||
            (f.selector || '').includes(item.where) ||
            (f.detail || '').includes(item.where)),
      )
      rows.push({
        ...item,
        fired: hits.length > 0,
        detail: hits[0]?.selector || (firedClasses.has(item.cls) ? '(class fired on a different cell)' : '(no finding)'),
      })
    } else if (item.kind === 'lint') {
      const r = runLint(item.lint, item.extra)
      rows.push({ ...item, fired: r.fired, detail: `exit ${r.exit}` })
    } else {
      rows.push({ ...item, fired: null, detail: 'vision rubric — not machine-gated' })
    }
  }

  // Report table.
  const w = (s, n) => String(s).padEnd(n)
  console.log(w('miss', 7) + w('class', 7) + w('kind', 10) + w('status', 9) + 'detail')
  console.log('─'.repeat(96))
  let failed = 0
  for (const r of rows) {
    const status =
      r.fired === null ? 'VISION' : r.fired ? 'FIRES ✓' : 'MISSING ✗'
    if (r.fired === false) failed++
    console.log(w(r.miss, 7) + w(r.cls, 7) + w(r.kind, 10) + w(status, 9) + `${r.desc} — ${r.detail}`)
  }

  const gated = rows.filter(r => r.fired !== null)
  const firing = gated.filter(r => r.fired).length
  console.log('─'.repeat(96))
  console.log(
    `\n${firing}/${gated.length} machine detectors FIRE on their known-bad instance` +
      ` (+${rows.length - gated.length} vision-only, not machine-gated).`,
  )

  if (JSON_OUT) {
    fs.writeFileSync(path.resolve(UI_DIR, JSON_OUT), JSON.stringify(rows, null, 2))
    console.log(`  → ${JSON_OUT}`)
  }

  if (failed) {
    console.error(`\n❌ DETECTOR-ACCEPTANCE FAILED — ${failed} detector(s) did NOT fire on their known bug.`)
    process.exit(1)
  }
  console.log('\n✅ DETECTOR-ACCEPTANCE PASSED — every machine detector fires on its known-bad instance.')
}

main().catch(e => {
  console.error(e)
  process.exit(2)
})
