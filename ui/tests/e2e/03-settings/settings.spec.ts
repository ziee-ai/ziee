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

    await assertNoAccessibilityViolations(page)
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

    await assertNoAccessibilityViolations(page)
  })

  test('should display theme selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')

    await expect(page.locator('text=Appearance')).toBeVisible()
    await expect(page.locator('text=Theme')).toBeVisible()
  })

  test('should change theme using selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')

    const themeSelect = page.locator('.ant-select').first()
    await themeSelect.click()
    await page.click('text=Dark')
    await page.waitForTimeout(500)

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)
  })
})
