import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad } from './helpers/navigation-helpers'

test.describe('Settings - General', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Ant Design Select has a known issue where aria-label doesn't pass through to the internal input
    // See: https://github.com/ant-design/ant-design/issues
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['label'], // Disable label rule due to AntD Select limitation
    })
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    await setTheme(page, 'dark')
    await waitForSettingsPageLoad(page, 'General')

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Ant Design Select has a known issue where aria-label doesn't pass through to the internal input
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['label'], // Disable label rule due to AntD Select limitation
    })
  })

  test('should display theme selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    await expect(page.getByText('Appearance')).toBeVisible()
    // Testing #theme-form selector - will dump prettified HTML on failure
    await expect(page.locator('#theme-form [aria-label="Theme"]').first()).toBeVisible()
  })

  test('should change theme using selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Testing #theme-form selector - will dump prettified HTML on failure
    await page.locator('#theme-form [aria-label="Theme"]').first().click()

    // Wait for dropdown to appear - Ant Design has hidden listbox, need CSS fallback for visible wrapper
    await page.getByRole('listbox').or(page.locator('.ant-select-dropdown')).first().waitFor({ state: 'visible' })

    // Select Dark option - use semantic selector (getByTitle)
    const darkOption = page.getByTitle('Dark')
    await darkOption.waitFor({ state: 'visible', timeout: 5000 })
    await darkOption.click()

    await page.waitForTimeout(500)

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)
  })
})
