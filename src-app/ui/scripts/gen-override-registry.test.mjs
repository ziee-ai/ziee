/**
 * TEST-7 — the override manifest drift guard (ITEM-10/ITEM-12).
 * Run: node --test scripts/gen-override-registry.test.mjs
 * Tests the pure `computeDrift`; the fs scan is exercised for real by
 * `npm run check:override-registry` every build.
 */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { computeDrift, topLevelSeamKeys } from './gen-override-registry.mjs'

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
