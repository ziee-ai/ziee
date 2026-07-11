/**
 * TEST-4 — the `.desktop.tsx` resolver tiers (ITEM-5).
 *
 * Exercises the pure `resolveOverridePath` over a real fixture tree, proving the
 * three-tier precedence (desktop-tree shadow → core-tree `.desktop.*` → core
 * base) and the null cases. Lives under `src/` so vitest's `src/**` glob picks
 * it up; the function under test is the plugin's exported pure core.
 */
import { describe, test, expect, beforeAll, afterAll } from 'vitest'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { resolveOverridePath } from '../../plugins/vite-plugin-local-override'

let root: string
let localSrc: string
let fallbackSrc: string

const opts = () => ({ localSrc, fallbackSrc, aliasPrefix: '@/' })
const touch = (p: string) => {
  mkdirSync(path.dirname(p), { recursive: true })
  writeFileSync(p, '// fixture')
}

beforeAll(() => {
  root = mkdtempSync(path.join(tmpdir(), 'seam-resolver-'))
  localSrc = path.join(root, 'desktop')
  fallbackSrc = path.join(root, 'core')

  // tier 3 only — core base
  touch(path.join(fallbackSrc, 'onlyCore.tsx'))
  // tier 2 wins over tier 3 — a core `.desktop.*` shadows the core base
  touch(path.join(fallbackSrc, 'withDesktop.tsx'))
  touch(path.join(fallbackSrc, 'withDesktop.desktop.tsx'))
  // tier 1 wins over all — a desktop-tree shadow beats both core files
  touch(path.join(localSrc, 'shadowed.tsx'))
  touch(path.join(fallbackSrc, 'shadowed.desktop.tsx'))
  touch(path.join(fallbackSrc, 'shadowed.tsx'))
})

afterAll(() => rmSync(root, { recursive: true, force: true }))

describe('resolveOverridePath', () => {
  test('tier 3: falls back to the core base file', () => {
    expect(resolveOverridePath('@/onlyCore', opts())).toBe(
      path.join(fallbackSrc, 'onlyCore.tsx'),
    )
  })

  test('tier 2: a core-tree `.desktop.*` shadows the core base', () => {
    expect(resolveOverridePath('@/withDesktop', opts())).toBe(
      path.join(fallbackSrc, 'withDesktop.desktop.tsx'),
    )
  })

  test('tier 1: a desktop-tree shadow wins over core base AND core `.desktop.*`', () => {
    expect(resolveOverridePath('@/shadowed', opts())).toBe(
      path.join(localSrc, 'shadowed.tsx'),
    )
  })

  test('non-`@/` specifiers are not handled (returns null)', () => {
    expect(resolveOverridePath('react', opts())).toBeNull()
    expect(resolveOverridePath('@tauri-apps/api', opts())).toBeNull()
  })

  test('an `@/` specifier with no matching file returns null', () => {
    expect(resolveOverridePath('@/does/not/exist', opts())).toBeNull()
  })
})
