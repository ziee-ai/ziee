import { test, expect } from '../../fixtures/test-context'
import { isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad, selectThemeOption } from './helpers/navigation-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the REAL ThemeSettings "Appearance" UI (ThemeSettings.tsx). The existing
 * settings dark-mode tests call the `setTheme` test util, which writes the
 * store/localStorage directly and BYPASSES the UI control. This drives the
 * actual Theme <Select> (aria-label "Theme") the user interacts with and
 * asserts the dark class is applied / removed.
 */

test.describe('Settings — Appearance theme switcher (real UI)', () => {
  test('selecting Dark then Light toggles the document theme class', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    const themeSelect = byTestId(page, 'settingsgen-theme-select')
    await expect(themeSelect).toBeVisible({ timeout: 15000 })

    // Choose Dark via the UI → the document gains the `dark` class.
    await selectThemeOption(page, 'dark')
    await expect.poll(() => isDarkMode(page), { timeout: 10000 }).toBe(true)

    // Choose Light via the UI → the `dark` class is removed.
    await selectThemeOption(page, 'light')
    await expect.poll(() => isDarkMode(page), { timeout: 10000 }).toBe(false)
  })
})
