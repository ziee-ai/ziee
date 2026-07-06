/**
 * EVALUATOR GATE (desktop) — the single UI-surface exit condition for the
 * desktop workspace. Mirrors the web workspace's `scripts/gate-ui.mjs`, adapted
 * to the desktop gallery's reality: it is PAGE-focused (it seeds every desktop
 * route surface via the mock API), and the kit COMPONENT stories — plus the
 * Layer-A/B pixel-visual specs that scan `gallery-section-*` — live only in the
 * web workspace. So the desktop gate drops the kit-visual step and asserts
 * coverage the desktop way (the generated coverage manifest is in sync).
 *
 * A desktop UI surface is DONE only if ALL of the following hold. This script
 * runs each check, prints a per-surface PASS/FAIL table, and exits non-zero on
 * any failure.
 *
 *   1. tsc       — `tsc --noEmit` is clean (types compile).
 *   2. lint      — biome guardrails + hardcoded-color lint are clean.
 *   3. runtime   — the runtime-health pass reports ZERO HIGH findings for every
 *                  page surface (no console error / pageerror / failed request /
 *                  crash / WCAG-AA contrast failure).
 *   4. coverage  — the generated gallery-coverage manifest is up to date
 *                  (`gen-gallery-coverage.mjs --check`), so no route surface
 *                  silently escapes the gallery.
 *
 * Usage:
 *   npm run gate:ui                 # full gate
 *   npm run gate:ui -- --skip-coverage   # runtime + tsc + lint only (fast)
 */
import { spawn, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const GALLERY_DIR = path.resolve(UI_DIR, 'src/dev/gallery')
const PORT = Number(process.env.GALLERY_PORT || 1420)
const SKIP_COVERAGE = process.argv.includes('--skip-coverage')

const results = [] // { name, ok, detail }
const step = (name, ok, detail = '') => {
  results.push({ name, ok, detail })
  console.log(`${ok ? '✅' : '❌'} ${name}${detail ? ` — ${detail}` : ''}`)
}

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, {
    cwd: UI_DIR,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    ...opts,
  })
  return { code: r.status ?? 1, out: (r.stdout || '') + (r.stderr || '') }
}

// HTTP health check against the gallery entry (matches how the pass reaches it —
// a TCP probe can miss a vite server bound only to IPv6 localhost).
const galleryUp = async port => {
  try {
    const ac = new AbortController()
    const t = setTimeout(() => ac.abort(), 1500)
    const r = await fetch(`http://localhost:${port}/dev-gallery.html`, {
      signal: ac.signal,
    })
    clearTimeout(t)
    return r.ok
  } catch {
    return false
  }
}

async function waitForPort(port, timeoutMs) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    if (await galleryUp(port)) return true
    await new Promise(r => setTimeout(r, 500))
  }
  return false
}

async function main() {
  console.log('=== desktop UI evaluator gate ===\n')

  // 1. tsc -------------------------------------------------------------------
  console.log('• typecheck (tsc --noEmit) …')
  const tsc = run('npx', ['tsc', '--noEmit'])
  step(
    'tsc',
    tsc.code === 0,
    tsc.code === 0 ? 'clean' : `${(tsc.out.match(/error TS/g) || []).length} errors`,
  )

  // 2. lint ------------------------------------------------------------------
  console.log('• lint (guardrails + colors) …')
  const guard = run('npm', ['run', 'lint:guardrails'])
  const colors = run('npm', ['run', 'lint:colors'])
  step(
    'lint',
    guard.code === 0 && colors.code === 0,
    guard.code === 0 && colors.code === 0 ? 'clean' : 'violations (see output above)',
  )
  if (guard.code !== 0) console.log(guard.out.slice(-1500))
  if (colors.code !== 0) console.log(colors.out.slice(-1500))

  // 3. runtime needs the gallery Vite server. Boot it (or reuse). ------------
  let vite = null
  const alreadyUp = await galleryUp(PORT)
  if (!alreadyUp) {
    console.log(`• booting gallery dev server on :${PORT} …`)
    vite = spawn(
      'npm',
      ['run', 'dev', '--', '--port', String(PORT), '--strictPort'],
      { cwd: UI_DIR, stdio: 'ignore', detached: false },
    )
    const up = await waitForPort(PORT, 120_000)
    if (!up) {
      step('gallery-server', false, `did not come up on :${PORT}`)
      finish()
      return
    }
  } else {
    console.log(`• reusing gallery dev server already on :${PORT}`)
  }

  try {
    // 3. runtime-health --------------------------------------------------------
    console.log('• runtime-health pass …')
    const rt = run('node', ['scripts/runtime-health.mjs', '--report-only'], {
      env: { ...process.env, GALLERY_PORT: String(PORT) },
    })
    console.log(rt.out.split('\n').slice(-8).join('\n'))
    const surfaceVerdicts = readRuntimeSurfaceVerdicts()
    const runtimeFail = surfaceVerdicts.filter(s => !s.ok)
    step(
      'runtime-health',
      rt.code === 0 && runtimeFail.length === 0,
      runtimeFail.length
        ? `${runtimeFail.length} surface(s) with HIGH findings`
        : `${surfaceVerdicts.length} surfaces clean`,
    )

    // 4. coverage manifest in sync --------------------------------------------
    if (SKIP_COVERAGE) {
      step('coverage', true, 'skipped (--skip-coverage)')
    } else {
      console.log('• gallery-coverage manifest …')
      const cov = run('npm', ['run', 'check:gallery-coverage'])
      step(
        'coverage',
        cov.code === 0,
        cov.code === 0 ? 'in sync' : 'stale — run `npm run gen:gallery-coverage`',
      )
      if (cov.code !== 0) console.log(cov.out.slice(-1500))
    }

    printSurfaceTable(surfaceVerdicts)
  } finally {
    if (vite) {
      try {
        vite.kill('SIGTERM')
      } catch {
        /* ignore */
      }
    }
  }

  finish()
}

/** Roll the runtime JSONL up into a per-surface PASS/FAIL (fail iff any HIGH). */
function readRuntimeSurfaceVerdicts() {
  const p = path.join(GALLERY_DIR, 'RUNTIME_FINDINGS.jsonl')
  if (!fs.existsSync(p)) return []
  const surfaces = {}
  for (const line of fs.readFileSync(p, 'utf8').split('\n').filter(Boolean)) {
    let f
    try {
      f = JSON.parse(line)
    } catch {
      continue
    }
    const s = (surfaces[f.surface] ??= { high: 0, medium: 0, low: 0, baselined: 0 })
    // A documented-baselined HIGH (runtime-baseline.js) does not fail a surface.
    if (f.baselined) s.baselined++
    else s[f.severity.toLowerCase()]++
  }
  return Object.entries(surfaces)
    .map(([surface, c]) => ({ surface, ...c, ok: c.high === 0 }))
    .sort((a, b) => b.high - a.high || a.surface.localeCompare(b.surface))
}

function printSurfaceTable(verdicts) {
  if (!verdicts.length) return
  const fails = verdicts.filter(v => !v.ok)
  console.log(
    `\n--- per-surface runtime verdict: ${verdicts.length - fails.length}/${verdicts.length} PASS ---`,
  )
  if (fails.length) {
    for (const v of fails)
      console.log(`   ❌ ${v.surface}  (HIGH ${v.high}, MEDIUM ${v.medium})`)
  } else {
    console.log('   ✅ all surfaces runtime-clean')
  }
}

function finish() {
  const failed = results.filter(r => !r.ok)
  console.log('\n=== gate summary ===')
  for (const r of results) console.log(`  ${r.ok ? 'PASS' : 'FAIL'}  ${r.name}`)
  if (failed.length) {
    console.log(`\n❌ GATE FAILED — ${failed.map(f => f.name).join(', ')}`)
    process.exit(1)
  }
  console.log('\n✅ GATE PASSED — every desktop UI DONE criterion met')
  process.exit(0)
}

main().catch(e => {
  console.error(e)
  process.exit(2)
})
