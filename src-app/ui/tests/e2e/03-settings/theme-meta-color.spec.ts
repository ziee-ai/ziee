import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { setTheme, isDarkMode } from '../../utils/theme'

/**
 * E2E — ThemeProvider drives the `<meta name="theme-color">` tag + the
 * documentElement dark/light class, and `useTheme()` consumers react to a
 * preference change without reload.
 *
 * Asserts that switching dark↔light flips the root class AND changes the
 * meta theme-color content (set from `currentTheme.token.colorBgContainer`).
 */

async function metaThemeColor(page: import('@playwright/test').Page) {
  return page.evaluate(
    () =>
      document
        .querySelector('meta[name="theme-color"]')
        ?.getAttribute('content') ?? null,
  )
}

test.describe('Theme — meta theme-color + useTheme reactivity', () => {
  test('switching dark/light flips the root class and the meta theme-color', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // Force dark and let ThemeProvider's effect run.
    await setTheme(page, 'dark')
    await expect.poll(() => isDarkMode(page)).toBe(true)
    const darkMeta = await metaThemeColor(page)
    expect(darkMeta).toBeTruthy()

    // Switch to light → the consumer reacts: root class flips and the meta
    // theme-color content changes to the light surface color.
    await setTheme(page, 'light')
    await expect.poll(() => isDarkMode(page)).toBe(false)
    const lightMeta = await metaThemeColor(page)
    expect(lightMeta).toBeTruthy()

    // The two themes must produce DIFFERENT theme-color values (proves the
    // meta tag is driven reactively by the active theme token, not static).
    expect(lightMeta).not.toBe(darkMeta)
  })
})
