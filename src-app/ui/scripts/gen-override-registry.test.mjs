/**
 * TEST-7 — the override manifest drift guard (ITEM-10/ITEM-12).
 * Run: node --test scripts/gen-override-registry.test.mjs
 * Tests the pure `computeDrift`; the fs scan is exercised for real by
 * `npm run check:override-registry` every build.
 */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  computeDrift,
  topLevelSeamKeys,
  parseShadowExceptions,
} from './gen-override-registry.mjs'

const m = (pairs) => new Map(pairs)

test('TEST-7: a clean matched set has no drift', () => {
  const declared = m([['hardware.monitor-button', 'core.tsx']])
  const registered = m([['hardware.monitor-button', 'desk.tsx']])
  const desktopFiles = [{ file: 'LeftSidebar.desktop.tsx', hasSibling: true }]
  const d = computeDrift(declared, registered, desktopFiles)
  assert.deepEqual(d.deadOverrides, [])
  assert.deepEqual(d.orphanDesktopFiles, [])
})

test('TEST-7: a registerOverride for an undeclared seam is a dead override', () => {
  const d = computeDrift(m([]), m([['ghost.key', 'desk.tsx']]), [])
  assert.deepEqual(d.deadOverrides, ['ghost.key'])
})

test('TEST-7: a `.desktop` file with no core sibling is an orphan', () => {
  const orphan = { file: 'Stray.desktop.tsx', hasSibling: false }
  const d = computeDrift(m([]), m([]), [orphan])
  assert.deepEqual(d.orphanDesktopFiles, [orphan])
})

test('TEST-7: a declared-but-unregistered seam is reported, not failed', () => {
  const d = computeDrift(m([['web.only', 'core.tsx']]), m([]), [])
  assert.deepEqual(d.unregisteredSeams, ['web.only'])
  assert.deepEqual(d.deadOverrides, [])
})

test('TEST-12: parseShadowExceptions captures approved shadow paths incl. hyphens/slashes', () => {
  const decisions = `
- SHADOW-EXCEPTION: main.tsx — entry point [approved: user 2026-07-11]
- SHADOW-EXCEPTION: api-client/types.ts — generated [approved: user 2026-07-11]
- SHADOW-EXCEPTION: modules/memory/module.tsx — glob module [approved: user 2026-07-11]
- SHADOW-EXCEPTION: not-approved/file.ts — reason with no approval token
`
  const set = parseShadowExceptions(decisions)
  assert.ok(set.has('main.tsx'))
  assert.ok(set.has('api-client/types.ts')) // hyphen in path captured fully
  assert.ok(set.has('modules/memory/module.tsx'))
  // an exception WITHOUT an [approved: …] token is NOT accepted
  assert.ok(!set.has('not-approved/file.ts'))
})

test('TEST-12: exceptions live in the PERMANENT OVERRIDE_EXCEPTIONS.md (survives .lifecycle strip)', () => {
  // The gate must NOT read approvals from .lifecycle/ (stripped at merge) — else
  // it false-fails on main forever. Source of truth is the committed product-tree
  // file; assert it exists and yields the 3 approved shadow paths.
  const here = path.dirname(fileURLToPath(import.meta.url))
  const permanent = path.join(here, '../../desktop/ui/OVERRIDE_EXCEPTIONS.md')
  assert.ok(fs.existsSync(permanent), 'OVERRIDE_EXCEPTIONS.md must exist in the product tree')
  const set = parseShadowExceptions(fs.readFileSync(permanent, 'utf-8'))
  for (const p of ['main.tsx', 'api-client/types.ts', 'modules/memory/module.tsx']) {
    assert.ok(set.has(p), `${p} must be an approved exception in OVERRIDE_EXCEPTIONS.md`)
  }
})

test('TEST-12: the raw-shadow gate flags an unaccounted shadow', () => {
  const shadows = ['main.tsx', 'modules/new-raw-override.tsx']
  const approved = parseShadowExceptions(
    '- SHADOW-EXCEPTION: main.tsx — entry [approved: user 2026-07-11]',
  )
  const unaccounted = shadows.filter((s) => !approved.has(s))
  assert.deepEqual(unaccounted, ['modules/new-raw-override.tsx'])
})

test('TEST-7: topLevelSeamKeys handles multi-line object seam values + ignores nested keys', () => {
  const src = `declare module '@/core/overrides' {
  interface UIOverrides {
    'layout.drawer-header': {
      'title': string
      onClose: () => void
    }
    'hardware.monitor-button': Record<string, never>
  }
}`
  const keys = topLevelSeamKeys(src)
  // both top-level seams captured; the NESTED 'title' key is NOT a seam
  assert.deepEqual(keys.sort(), ['hardware.monitor-button', 'layout.drawer-header'])
})
