/**
 * TEST-5 — the seam codemod's transforms (ITEM-7).
 * Run: node --test scripts/seam-codemod.test.mjs
 */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  slugForKey,
  insertAugmentation,
  registrationStub,
  classifyDivergence,
} from './seam-codemod.mjs'

test('TEST-5: slugForKey turns a dotted key into a file slug', () => {
  assert.equal(slugForKey('hardware.monitor-button'), 'hardware-monitor-button')
})

test('TEST-5: insertAugmentation inserts the seam decl after the last import', () => {
  const src = `import { Button } from '@/components/ui'\nimport { x } from './x'\n\nexport function Foo() { return null }\n`
  const { src: out, changed } = insertAugmentation(src, 'foo.bar')
  assert.equal(changed, true)
  assert.match(out, /interface UIOverrides/)
  assert.match(out, /'foo\.bar':/)
  // the augmentation lands AFTER the imports, BEFORE the component
  assert.ok(out.indexOf("interface UIOverrides") > out.indexOf("from './x'"))
  assert.ok(out.indexOf('interface UIOverrides') < out.indexOf('export function Foo'))
})

test('TEST-5: insertAugmentation is idempotent when the key is already declared', () => {
  const src = `import x from 'y'\ndeclare module '@/core/overrides' { interface UIOverrides { 'foo.bar': Record<string, never> } }\n`
  const { changed } = insertAugmentation(src, 'foo.bar')
  assert.equal(changed, false)
})

test('TEST-5: registrationStub emits a register() calling registerOverride for the key', () => {
  const stub = registrationStub('hardware.monitor-button')
  assert.match(stub, /export function register\(\): void/)
  assert.match(stub, /registerOverride\('hardware\.monitor-button'/)
  assert.match(stub, /function DesktopHardwareMonitorButton/)
})

test('TEST-5: classifyDivergence flags a localized diff as element-level', () => {
  const core = Array.from({ length: 20 }, (_, i) => `line ${i}`).join('\n')
  const shadow = core.replace('line 5', 'line 5 CHANGED')
  assert.equal(classifyDivergence(core, shadow).classification, 'element-level')
})

test('TEST-5: classifyDivergence flags a pervasive diff as structural', () => {
  const core = Array.from({ length: 20 }, (_, i) => `core ${i}`).join('\n')
  const shadow = Array.from({ length: 20 }, (_, i) => `desktop ${i}`).join('\n')
  assert.equal(classifyDivergence(core, shadow).classification, 'structural')
})
