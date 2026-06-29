import { test, expect } from '../../fixtures/test-context'
import { isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToSettingsPage,
  waitForSettingsPageLoad,
  selectThemeOption,
} from './helpers/navigation-helpers'

/**
 * E2E — the "System" theme option follows the OS color-scheme.
 *
 * Audit gap: settings.spec.ts covers picking "Dark" explicitly, but the
 * `system` option (ThemeSettings.tsx — match the OS) was never tested.
 * This selects System, then flips the emulated OS color-scheme and asserts
 * the app's `dark` class tracks it (dark → light), proving System is wired
 * to the media query rather than a fixed value.
 */

test.describe('Settings — System theme follows OS', () => {
  test('selecting System tracks the emulated OS color-scheme', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.emulateMedia({ colorScheme: 'dark' })

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Pick the "System" theme option.
    await selectThemeOption(page, 'system')
    await page.waitForTimeout(500)

    // OS is dark → app is dark.
    expect(await isDarkMode(page)).toBe(true)

    // Flip the OS to light → System theme must follow.
    await page.emulateMedia({ colorScheme: 'light' })
    await expect.poll(() => isDarkMode(page), { timeout: 5000 }).toBe(false)
  })
})
