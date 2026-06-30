/**
 * Interactive-state coverage — hover + keyboard focus. The audit found NO
 * `.hover()`/`.focus()` snapshots anywhere, so focus rings and hover micro-states
 * (exactly the things that silently regress) were untested.
 *
 * For each target we drive the real pseudo-state with Playwright, assert the
 * focused element still passes the layout invariants (a focus ring must not cause
 * overflow), and snapshot it. Backend-free via the gallery Vite server.
 */
import { expect, test } from '@playwright/test'
import { assertLayoutSane } from '../helpers/layout'
import { SNAPSHOTS_ENABLED, openGallery } from './_gallery'

interface StateCase {
  testid: string
  /** which pseudo-states to exercise. */
  states: Array<'hover' | 'focus'>
}

const TARGETS: StateCase[] = [
  { testid: 'g-btn-default-default', states: ['hover', 'focus'] },
  { testid: 'g-btn-outline-default', states: ['hover', 'focus'] },
  { testid: 'g-btn-destructive-default', states: ['hover', 'focus'] },
  { testid: 'g-input-default', states: ['focus'] },
  { testid: 'g-sel-filled', states: ['focus'] },
  { testid: 'g-sw-off', states: ['focus'] },
  { testid: 'g-cb-off', states: ['focus'] },
  { testid: 'g-tag-closable', states: ['hover'] },
  { testid: 'g-tooltip-trigger', states: ['hover'] },
]

test('interactive states — hover + focus (light)', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 })
  await openGallery(page, 'light', 'blue')

  for (const t of TARGETS) {
    const el = page.getByTestId(t.testid)
    for (const state of t.states) {
      await test.step(`${t.testid}:${state}`, async () => {
        await el.scrollIntoViewIfNeeded()
        if (state === 'hover') {
          await el.hover()
        } else {
          await el.focus()
        }
        // A focus ring / hover elevation must not push the element out of its box.
        await assertLayoutSane(el, {
          checks: { horizontalScroll: false, touchTarget: false },
        })
        if (SNAPSHOTS_ENABLED) {
          await expect(el).toHaveScreenshot(`state-${t.testid}-${state}.png`)
        }
        // Reset hover/focus before the next target.
        await page.mouse.move(0, 0)
        await el.blur().catch(() => undefined)
      })
    }
  }
})
