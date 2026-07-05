import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — clicking the mobile overlay mask closes the sidebar.
 *
 * On an xs viewport the sidebar is a Sheet (Base-UI Dialog) that portals to
 * <body>; its backdrop is the `[data-slot="sheet-overlay"]` element. Clicking
 * the backdrop dismisses the Dialog (open→closed), which unmounts the Sheet
 * content — so open/closed is asserted via visibility of the sidebar node.
 */

test.use({ viewport: { width: 390, height: 844 } })

test.describe('Layout — mobile sidebar mask', () => {
  test('clicking the mask closes the open mobile sidebar', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/profile`)

    const sidebar = byTestId(page, 'app-sidebar')
    // Mobile: the Sheet starts closed, so the sidebar content is not shown.
    await expect(sidebar).toBeHidden({ timeout: 30000 })

    // Open the overlay.
    await byTestId(page, 'layout-sidebar-toggle-button').click()
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Click the mask (Sheet backdrop) to dismiss. The overlay is full-screen,
    // but the panel occupies the left ~250px, so click well to the RIGHT of it
    // — a press that starts+ends outside the panel is what triggers the
    // Dialog's outside-press dismiss.
    await page
      .locator('[data-slot="sheet-overlay"]')
      .click({ position: { x: 340, y: 400 }, force: true })
    await expect(sidebar).toBeHidden({ timeout: 10000 })
  })
})
