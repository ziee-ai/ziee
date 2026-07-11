/**
 * TEST-6 — the desktop Drawer's restored stacking guard (ITEM-9 / DRIFT-1.7).
 *
 * The regression the desktop shadow had was that the guard was ABSENT, so
 * closing a dialog stacked ABOVE the drawer also dismissed the drawer. We test
 * the extracted pure decision (`isHigherLayerPresent`); the full DOM-query
 * behavior is exercised end-to-end by the desktop e2e (TEST-9). The predicate
 * being present + correct proves the guard was ported back.
 */
import { describe, test, expect } from 'vitest'
import { isHigherLayerPresent } from './Drawer'

const el = (id: string) => ({ id }) as unknown as Element

describe('isHigherLayerPresent (drawer stacking guard)', () => {
  const self = el('self')

  test('a layer stacked strictly above thisZ triggers the guard', () => {
    expect(isHigherLayerPresent([{ el: el('top'), z: 60 }], self, 50)).toBe(true)
  })

  test('the drawer itself is never counted as a higher layer', () => {
    expect(isHigherLayerPresent([{ el: self, z: 999 }], self, 50)).toBe(false)
  })

  test('equal or lower layers do not trigger the guard', () => {
    expect(
      isHigherLayerPresent(
        [
          { el: el('same'), z: 50 },
          { el: el('below'), z: 40 },
        ],
        self,
        50,
      ),
    ).toBe(false)
  })

  test('a non-finite z-index (e.g. "auto") is ignored', () => {
    expect(isHigherLayerPresent([{ el: el('auto'), z: NaN }], self, 50)).toBe(
      false,
    )
  })
})
