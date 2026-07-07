/**
 * TEST-6 — overlay-registry gate. Asserts the desktop workspace owns the overlay
 * registry generator + manifest + allowlist + committed generated registry, that
 * `check:overlay-registry` is chained into `check`, and that `--check` PASSES
 * (every desktop-only overlay host is wired open or allowlisted). Runs the REAL
 * generator.
 */
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const WS = path.resolve(HERE, '../../..') // desktop/ui
const has = (p: string) => fs.existsSync(path.join(WS, p))
const pkg = JSON.parse(fs.readFileSync(path.join(WS, 'package.json'), 'utf8'))

describe('TEST-6: overlay-registry gate', () => {
  test('generator + manifest + allowlist + generated registry present', () => {
    expect(has('scripts/gen-overlay-registry.mjs')).toBe(true)
    expect(has('src/dev/gallery/overlays.tsx')).toBe(true)
    expect(has('src/dev/gallery/overlay-allowlist.json')).toBe(true)
    expect(has('src/dev/gallery/overlay-registry.generated.json')).toBe(true)
  })

  test('check:overlay-registry chained into check', () => {
    expect(pkg.scripts.check).toContain('run check:overlay-registry')
    expect(pkg.scripts['check:overlay-registry']).toContain('--check')
  })

  test('gen-overlay-registry.mjs --check exits 0', () => {
    const out = execFileSync('node', ['scripts/gen-overlay-registry.mjs', '--check'], {
      cwd: WS,
      encoding: 'utf8',
    })
    expect(out).toMatch(/overlay gate OK/)
  })

  test('overlay-allowlist.json parses and every host has a non-empty reason', () => {
    const j = JSON.parse(
      fs.readFileSync(path.join(WS, 'src/dev/gallery/overlay-allowlist.json'), 'utf8'),
    )
    for (const [host, reason] of Object.entries(j.hosts ?? {})) {
      expect(typeof reason, `${host} reason is a string`).toBe('string')
      expect((reason as string).length, `${host} reason non-empty`).toBeGreaterThan(20)
    }
  })
})
