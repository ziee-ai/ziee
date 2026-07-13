import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { isDarkMode } from '../../utils/theme'

// TEST-3 (covers ITEM-1, ITEM-4): the SAME shared AuthThemeToggle works in the
// setup context. A REAL click on `auth-theme-toggle` flips the `<html>`
// dark class, and the setup form/card survive the flip.
test.describe('Setup theme toggle', () => {
  test('a real click flips the theme and keeps the form', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    const toggle = byTestId(page, 'auth-theme-toggle')
    await expect(toggle).toBeVisible()

    const before = await isDarkMode(page)
    await toggle.click()
    await expect.poll(() => isDarkMode(page)).toBe(!before)

    // The setup card + form are still there after the theme flip.
    await expect(byTestId(page, 'app-setup-card')).toBeVisible()
    await expect(byTestId(page, 'app-setup-form')).toBeVisible()
  })
})
