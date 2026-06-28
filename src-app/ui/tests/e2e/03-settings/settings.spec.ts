import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode, getTheme } from '../../utils/theme'
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

  // audit id 70b1e4252cdd120f — theme persistence across a session boundary.
  // ConfigClient.store persists themePreference to localStorage
  // (config-client-storage); the existing tests only assert the in-session
  // toggle. This selects Dark via the UI, RELOADS (re-bootstrap + store
  // rehydrate), and asserts dark mode survives.
  test('theme preference persists across a page reload', async ({ page, testInfra }) => {
  test('selecting Light / System persists the preference across a reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Choose Dark through the real selector.
    await page.locator('#theme-form [aria-label="Theme"]').first().click()
    await page
      .getByRole('listbox')
      .or(page.locator('.ant-select-dropdown'))
      .first()
      .waitFor({ state: 'visible' })
    const darkOption = page.getByTitle('Dark')
    await darkOption.waitFor({ state: 'visible', timeout: 5000 })
    await darkOption.click()
    await page.waitForTimeout(500)
    expect(await isDarkMode(page)).toBe(true)

    // Reload → the app re-bootstraps and the store rehydrates from
    // localStorage; dark mode must still be active.
    await page.reload({ waitUntil: 'load' })
    await expect.poll(() => isDarkMode(page), { timeout: 10000 }).toBe(true)
    expect(await getTheme(page)).toBe('dark')
    const pickTheme = async (label: 'Light' | 'Dark' | 'System') => {
      await page.locator('#theme-form [aria-label="Theme"]').first().click()
      await page
        .getByRole('listbox')
        .or(page.locator('.ant-select-dropdown'))
        .first()
        .waitFor({ state: 'visible' })
      const opt = page.getByTitle(label)
      await opt.waitFor({ state: 'visible', timeout: 5000 })
      await opt.click()
      await page.waitForTimeout(300)
    }

    // Light → not dark, and the preference persists across a reload.
    await pickTheme('Light')
    expect(await isDarkMode(page)).toBe(false)
    expect(await getTheme(page)).toBe('light')
    await page.reload()
    await waitForSettingsPageLoad(page, 'General')
    expect(await getTheme(page)).toBe('light')
    expect(await isDarkMode(page)).toBe(false)

    // System → the stored preference is 'system' (and survives a reload).
    await pickTheme('System')
    expect(await getTheme(page)).toBe('system')
    await page.reload()
    await waitForSettingsPageLoad(page, 'General')
    expect(await getTheme(page)).toBe('system')
  })
})
