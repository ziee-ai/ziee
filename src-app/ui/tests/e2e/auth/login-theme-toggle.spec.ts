import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { isDarkMode, getTheme } from '../../utils/theme'
import { createAdminViaSetup } from './helpers/form-helpers'
import { logoutAndGoToAuth } from './helpers/navigation-helpers'

// TEST-1 (covers ITEM-1, ITEM-3): the login page now has a before-sign-in theme
// toggle. Prove it by a REAL click (not the localStorage setTheme helper): the
// `<html>` dark class flips, the choice persists, and it survives a reload.
test.describe('Login theme toggle', () => {
  test('a real click flips the theme, persists, and survives reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await createAdminViaSetup(page, baseURL)
    await logoutAndGoToAuth(page, baseURL)

    const toggle = byTestId(page, 'auth-theme-toggle')
    await expect(toggle).toBeVisible()

    const before = await isDarkMode(page)
    await toggle.click()
    await expect.poll(() => isDarkMode(page)).toBe(!before)

    // Persisted to the shared ConfigClient store (explicit light/dark, not 'system').
    expect(await getTheme(page)).toBe(!before ? 'dark' : 'light')

    // Survives a reload (the login form comes back with the chosen theme).
    await page.reload({ waitUntil: 'domcontentloaded' })
    await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })
    expect(await isDarkMode(page)).toBe(!before)

    // Toggling again returns to the original theme.
    await byTestId(page, 'auth-theme-toggle').click()
    await expect.poll(() => isDarkMode(page)).toBe(before)
  })
})
