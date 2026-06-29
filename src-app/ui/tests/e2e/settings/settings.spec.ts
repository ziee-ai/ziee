import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode, getTheme } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad, selectThemeOption } from './helpers/navigation-helpers'
import { byTestId } from '../testid.ts'

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

    await expect(byTestId(page, 'settingsgen-appearance-card')).toBeVisible()
    await expect(byTestId(page, 'settingsgen-theme-select')).toBeVisible()
  })

  test('should change theme using selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Drive the real Appearance theme Select (kit Select, testid-based).
    await selectThemeOption(page, 'dark')

    await page.waitForTimeout(500)

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)
  })

  test('selecting Light / System persists the preference across a reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    const pickTheme = async (value: 'light' | 'dark' | 'system') => {
      await selectThemeOption(page, value)
      await page.waitForTimeout(300)
    }

    // Light → not dark, and the preference persists across a reload.
    await pickTheme('light')
    expect(await isDarkMode(page)).toBe(false)
    expect(await getTheme(page)).toBe('light')
    await page.reload()
    await waitForSettingsPageLoad(page, 'General')
    expect(await getTheme(page)).toBe('light')
    expect(await isDarkMode(page)).toBe(false)

    // System → the stored preference is 'system' (and survives a reload).
    await pickTheme('system')
    expect(await getTheme(page)).toBe('system')
    await page.reload()
    await waitForSettingsPageLoad(page, 'General')
    expect(await getTheme(page)).toBe('system')
  })
})
