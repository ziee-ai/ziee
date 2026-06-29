import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — a multi-page settings navigation journey.
 *
 * Audit gap: settings-shell.spec only exercises a single page + single
 * action. This navigates ACROSS several settings sections via the settings
 * menu (client-side routing), asserting each target section renders — the
 * cross-page journey the shell is meant to support.
 */

test.describe('Settings — multi-page journey', () => {
  test('navigates Profile → General → Hardware via the settings menu', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // Profile → General (client-side route via the settings menu).
    await byTestId(page, 'settings-nav-menu-item-general').click()
    await expect(page).toHaveURL(/\/settings\/general$/)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    // General → Hardware.
    await byTestId(page, 'settings-nav-menu-item-hardware').click()
    await expect(page).toHaveURL(/\/settings\/hardware$/)
    await byTestId(page, 'settings-page-title').waitFor({
      state: 'visible',
      timeout: 30000,
    })

    // Hardware → back to Profile (the menu persists across pages).
    await byTestId(page, 'settings-nav-menu-item-profile').click()
    await expect(page).toHaveURL(/\/settings\/profile$/)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })
  })
})
