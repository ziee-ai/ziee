import { test, expect } from '../../fixtures/test-context'
import type { Page, Locator } from '@playwright/test'
import { byTestId } from '../testid'
import { createAdminViaSetup } from './helpers/form-helpers'
import { logoutAndGoToAuth } from './helpers/navigation-helpers'

// TEST-6 (covers ITEM-6): both unauthenticated pages are usable at a 390px
// mobile width — no horizontal page scroll, the card is visible, and the theme
// toggle is a ≥40px tap target that does not overlap the card.
test.describe('Auth pages at 390px (mobile)', () => {
  test.use({ viewport: { width: 390, height: 844 } })

  async function assertNoHorizontalScroll(page: Page) {
    const noOverflow = await page.evaluate(
      () =>
        document.documentElement.scrollWidth <=
        document.documentElement.clientWidth,
    )
    expect(noOverflow).toBe(true)
  }

  async function assertToggleTapTargetClearOfCard(
    toggle: Locator,
    card: Locator,
  ) {
    const t = await toggle.boundingBox()
    const c = await card.boundingBox()
    expect(t).not.toBeNull()
    expect(c).not.toBeNull()
    if (!t || !c) return
    expect(t.width).toBeGreaterThanOrEqual(40)
    expect(t.height).toBeGreaterThanOrEqual(40)
    // no rectangle intersection between the toggle and the card
    const overlaps =
      t.x < c.x + c.width &&
      t.x + t.width > c.x &&
      t.y < c.y + c.height &&
      t.y + t.height > c.y
    expect(overlaps).toBe(false)
  }

  test('login', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await createAdminViaSetup(page, baseURL)
    await logoutAndGoToAuth(page, baseURL)

    await assertNoHorizontalScroll(page)
    await expect(byTestId(page, 'auth-login-card')).toBeVisible()
    await assertToggleTapTargetClearOfCard(
      byTestId(page, 'auth-theme-toggle'),
      byTestId(page, 'auth-login-card'),
    )
  })

  test('setup', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    await assertNoHorizontalScroll(page)
    await expect(byTestId(page, 'app-setup-card')).toBeVisible()
    await assertToggleTapTargetClearOfCard(
      byTestId(page, 'app-setup-theme-toggle'),
      byTestId(page, 'app-setup-card'),
    )
  })
})
