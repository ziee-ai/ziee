/**
 * TEST-8 — seam parity for the converted shadow(s) (ITEM-8).
 *
 * Locks the behavior of the DELETED desktop shadow into the new mechanism: with
 * the desktop registration applied, the seam resolves to the desktop variant;
 * without it, the seam is unresolved so the core fallback renders. Parametrized
 * over every converted seam (currently just `hardware.monitor-button`; see
 * DRIFT-1.6 for why the other shadows stayed structural file-swaps).
 */
import { describe, test, expect, beforeEach } from 'vitest'
import { resolveOverride } from '@/core/overrides'
import { __clearOverrides } from '@/core/overrides/registry'
import { register as registerHardware } from './overrides/hardware-monitor'
import { register as registerSpacer } from './overrides/sidebar-header-spacer'

const CONVERTED_SEAMS: { key: string; register: () => void }[] = [
  { key: 'hardware.monitor-button', register: registerHardware },
  { key: 'layout.sidebar-header-spacer', register: registerSpacer },
]

describe('converted seam parity', () => {
  beforeEach(() => __clearOverrides())

  for (const { key, register } of CONVERTED_SEAMS) {
    test(`${key}: fallback when unregistered, desktop variant after register()`, () => {
      // Web behavior: no registration → seam falls back to the core element.
      expect(resolveOverride(key as never)).toBeUndefined()
      // Desktop behavior: the registration installs the desktop variant.
      register()
      const Variant = resolveOverride(key as never)
      expect(Variant).toBeTypeOf('function')
    })
  }
})
