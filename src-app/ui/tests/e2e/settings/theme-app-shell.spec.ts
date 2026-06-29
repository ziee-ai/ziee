import { test, expect } from '../../fixtures/test-context'
import { isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToSettingsPage,
  waitForSettingsPageLoad,
  selectThemeOption,
} from './helpers/navigation-helpers'
import { byTestId } from '../testid.ts'

/**
 * Theme changes propagate beyond the settings page into the app shell + chat
 * UI (ThemeProvider toggles the `dark` class on <html>). settings.spec only
 * asserts the change on the settings page itself; this follows the user to the
 * new-chat surface and back.
 */
test.describe('Theme — applies across the app shell', () => {
  test('selecting Dark on settings carries into the chat shell, Light reverts', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Pick Dark via the real theme selector.
    await selectThemeOption(page, 'dark')
    await page.waitForTimeout(500)
    expect(await isDarkMode(page)).toBe(true)

    // Navigate to the app shell / new chat surface — the dark class persists.
    await page.goto(`${baseURL}/`)
    await expect(byTestId(page, 'app-sidebar')).toBeVisible({ timeout: 30000 })
    expect(await isDarkMode(page)).toBe(true)

    // Switch back to Light from settings → the chat shell goes light too.
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')
    await selectThemeOption(page, 'light')
    await page.waitForTimeout(500)

    await page.goto(`${baseURL}/`)
    await expect(byTestId(page, 'app-sidebar')).toBeVisible({ timeout: 30000 })
    expect(await isDarkMode(page)).toBe(false)
  })
})
