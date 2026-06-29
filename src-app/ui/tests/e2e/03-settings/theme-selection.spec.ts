import { test, expect } from '../../fixtures/test-context'
import { isDarkMode, getTheme } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { selectThemeOption } from './helpers/navigation-helpers'

// audit id all-514c7aa61b7c — the theme option must be selectable THROUGH THE
// UI (ThemeSettings.tsx:52-58 Select → ConfigClient.setThemePreference), not
// only via the localStorage seed the other theme tests use. This drives the
// real antd Select on /settings/general and asserts the choice both persists
// (localStorage) and takes visual effect (the `dark` class on <html>).
test.describe('Theme selection through the UI', () => {
  test('selecting Dark in the appearance Select switches the app to dark mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/general`)
    await expect(byTestId(page, 'settingsgen-appearance-card')).toBeVisible({
      timeout: 30000,
    })

    // Open the Theme select and pick "Dark".
    await selectThemeOption(page, 'dark')

    // The selection persists to the config-client store...
    await expect.poll(() => getTheme(page), { timeout: 10000 }).toBe('dark')
    // ...and visually applies the dark class to the document.
    await expect.poll(() => isDarkMode(page), { timeout: 10000 }).toBe(true)

    // Flip back to Light through the UI to prove the round-trip both ways.
    await selectThemeOption(page, 'light')
    await expect.poll(() => getTheme(page), { timeout: 10000 }).toBe('light')
    await expect.poll(() => isDarkMode(page), { timeout: 10000 }).toBe(false)
  })
})
