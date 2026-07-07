/**
 * Guardrail-parity tests (TEST-1/2/3/5) — assert the desktop workspace has
 * backfilled the server-ui static gates + audit tooling. These read the real
 * package.json + on-disk script/allowlist files (no mocks); they fail if a gate
 * is dropped from `check` or an audit script goes missing.
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const WS = path.resolve(HERE, '../../..') // desktop/ui
const WEB = path.resolve(WS, '../../ui')
const pkg = JSON.parse(fs.readFileSync(path.join(WS, 'package.json'), 'utf8'))
const exists = (p: string) => fs.existsSync(path.join(WS, p))

describe('TEST-1: backfilled static gates chained into `check`', () => {
  const chain = pkg.scripts.check as string
  for (const gate of [
    'lint:adjacent-inline',
    'lint:icon-action',
    'check:kit-manifest',
    'check:testid-registry',
    'check:design-spec',
    'check:overlay-registry',
  ]) {
    test(`check runs ${gate}`, () => {
      expect(chain).toContain(`run ${gate}`)
      expect(pkg.scripts[gate], `script ${gate} defined`).toBeTruthy()
    })
  }

  test('referenced ../../ui/scripts/*.mjs paths resolve on disk', () => {
    const refs = Object.values(pkg.scripts as Record<string, string>)
      .flatMap(s => [...s.matchAll(/\.\.\/\.\.\/ui\/scripts\/([\w.-]+\.mjs)/g)].map(m => m[1]))
    expect(refs.length).toBeGreaterThan(0)
    for (const r of new Set(refs)) {
      expect(fs.existsSync(path.join(WEB, 'scripts', r)), `web script ${r} exists`).toBe(true)
    }
  })
})

describe('TEST-2: geometry audit tooling', () => {
  test('gallery-geometry-audit.mjs is byte-identical to the web source', () => {
    const local = fs.readFileSync(path.join(WS, 'scripts/gallery-geometry-audit.mjs'), 'utf8')
    const web = fs.readFileSync(path.join(WEB, 'scripts/gallery-geometry-audit.mjs'), 'utf8')
    expect(local).toBe(web)
  })
  test('gallery:geometry + gallery:geometry:gate scripts defined', () => {
    expect(pkg.scripts['gallery:geometry']).toContain('gallery-geometry-audit.mjs')
    expect(pkg.scripts['gallery:geometry:gate']).toContain('--gate')
  })
  test('geometry-allowlist.json exists and parses', () => {
    const raw = fs.readFileSync(path.join(WS, 'src/dev/gallery/geometry-allowlist.json'), 'utf8')
    expect(() => JSON.parse(raw)).not.toThrow()
    expect(JSON.parse(raw)).toHaveProperty('entries')
  })
})

describe('TEST-3: affordance audit tooling', () => {
  test('affordance-audit.mjs + allowlist exist', () => {
    expect(exists('scripts/affordance-audit.mjs')).toBe(true)
    expect(exists('scripts/affordance-audit-allowlist.json')).toBe(true)
  })
  test('gallery:affordance script defined', () => {
    expect(pkg.scripts['gallery:affordance']).toContain('affordance-audit.mjs')
  })
})

describe('TEST-5: crop-review vision tooling', () => {
  test('gen-crop-review-manifests.mjs + docs/DEFECT_TAXONOMY.md exist', () => {
    expect(exists('scripts/gen-crop-review-manifests.mjs')).toBe(true)
    expect(exists('docs/DEFECT_TAXONOMY.md')).toBe(true)
  })
  test('gen:crop-review script defined', () => {
    expect(pkg.scripts['gen:crop-review']).toContain('gen-crop-review-manifests.mjs')
  })
  test('docs/DEFECT_TAXONOMY.md carries the [V] vision rubric the crop script parses', () => {
    // Full manifest generation needs a live gallery server (it enumerates
    // surfaces via Playwright) — that runs in the audit pass (ITEM-8), not here.
    // Statically assert the inputs: the script reads docs/DEFECT_TAXONOMY.md and
    // the taxonomy actually has `[V]` vision-class lines to parse.
    const script = fs.readFileSync(
      path.join(WS, 'scripts/gen-crop-review-manifests.mjs'),
      'utf8',
    )
    expect(script).toContain('docs/DEFECT_TAXONOMY.md')
    const taxonomy = fs.readFileSync(path.join(WS, 'docs/DEFECT_TAXONOMY.md'), 'utf8')
    expect(taxonomy).toMatch(/\[V\]/)
  })
})
