import { test, expect } from '../../fixtures/test-context'
import { isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad } from './helpers/navigation-helpers'

/**
 * E2E — ThemeSettings 'Light' and 'System' options (settings.spec only ever
 * selects 'Dark'). 'Light' must turn dark mode off; 'System' must follow the OS
 * `prefers-color-scheme` (emulated here) rather than a fixed value.
 */

async function selectTheme(page: import('@playwright/test').Page, label: string) {
  await page.locator('#theme-form [aria-label="Theme"]').first().click()
  await page
    .getByRole('listbox')
    .or(page.locator('.ant-select-dropdown'))
    .first()
    .waitFor({ state: 'visible' })
  await page.getByTitle(label, { exact: true }).click()
  await page.waitForTimeout(300)
}

test.describe('Settings — theme Light/System options', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToSettingsPage(page, testInfra.baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')
  })

  test('selecting Light disables dark mode', async ({ page }) => {
    // Start from Dark so the switch to Light is observable.
    await selectTheme(page, 'Dark')
    expect(await isDarkMode(page)).toBe(true)

    await selectTheme(page, 'Light')
    expect(await isDarkMode(page)).toBe(false)
  })

  test('selecting System follows the OS prefers-color-scheme', async ({ page }) => {
    // Emulate an OS dark preference → System theme must resolve to dark.
    await page.emulateMedia({ colorScheme: 'dark' })
    await selectTheme(page, 'System')
    await expect.poll(() => isDarkMode(page)).toBe(true)

    // Flip the OS preference to light → System resolves to light.
    await page.emulateMedia({ colorScheme: 'light' })
    await expect.poll(() => isDarkMode(page)).toBe(false)
  })
})
