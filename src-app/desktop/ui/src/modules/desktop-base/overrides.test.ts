/**
 * TEST-3 — the desktop override registration entry point (ITEM-4).
 *
 * Proves `registerDesktopOverrides()` (auto-discovering every `overrides/*` file's
 * `register()`) puts the expected seam keys into the shared registry, at
 * import/module-init time (no DOM), so they are present before the first render.
 */
import { describe, test, expect, beforeEach } from 'vitest'
import { registerDesktopOverrides } from './overrides'
import { resolveOverride } from '@/core/overrides'
import {
  __clearOverrides,
  __registeredOverrideKeys,
} from '@/core/overrides/registry'

describe('registerDesktopOverrides', () => {
  beforeEach(() => __clearOverrides())

  test('registers the hardware.monitor-button seam', () => {
    expect(resolveOverride('hardware.monitor-button' as never)).toBeUndefined()
    registerDesktopOverrides()
    expect(resolveOverride('hardware.monitor-button' as never)).toBeTypeOf(
      'function',
    )
  })

  test('every registered key is a non-empty `<module>.<element>` string', () => {
    registerDesktopOverrides()
    const keys = __registeredOverrideKeys()
    expect(keys.length).toBeGreaterThan(0)
    for (const k of keys) expect(k).toMatch(/^[a-z0-9-]+(\.[a-z0-9-]+)+$/)
  })
})
