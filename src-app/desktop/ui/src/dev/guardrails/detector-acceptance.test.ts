/**
 * TEST-4 — detector-acceptance meta-test. Asserts the desktop workspace owns the
 * detector scripts + copied fixtures, and that `detector-acceptance.mjs` PASSES:
 * the two lint detectors fire on the fixtures and the geometry detector is
 * byte-identical to the validated web source. Runs the REAL script (no mocks).
 */
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const WS = path.resolve(HERE, '../../..') // desktop/ui
const has = (p: string) => fs.existsSync(path.join(WS, p))

describe('TEST-4: detector-acceptance', () => {
  test('detector scripts + fixtures present', () => {
    expect(has('scripts/detector-acceptance.mjs')).toBe(true)
    expect(has('scripts/lint-icon-action.mjs')).toBe(true)
    expect(has('scripts/lint-native-scroll.mjs')).toBe(true)
    expect(has('src/dev/gallery/__detector_fixtures__')).toBe(true)
  })

  test('detector-acceptance.mjs exits 0 (lint detectors fire + geometry byte-identical)', () => {
    // Throws (non-zero exit) → test fails, surfacing which detector did not fire.
    const out = execFileSync('node', ['scripts/detector-acceptance.mjs'], {
      cwd: WS,
      encoding: 'utf8',
    })
    expect(out).toMatch(/DETECTOR-ACCEPTANCE PASSED/)
    expect(out).toMatch(/C11.*OK/)
    expect(out).toMatch(/J8.*OK/)
    expect(out).toMatch(/geometry-identity.*OK/)
  })
})
