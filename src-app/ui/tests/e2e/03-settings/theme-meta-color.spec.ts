import { test, expect } from '../../fixtures/test-context'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * ThemeProvider keeps the document <html> class (dark/light) AND the
 * <meta name="theme-color"> content in sync with the active theme (a useTheme()
 * consumer reacting to theme changes). This asserts both flip — and that the
 * meta theme-color content differs between dark and light (reactivity).
 */
test.describe('Theme — meta theme-color + html class reactivity', () => {
  test('switching dark/light updates html class + meta theme-color', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const metaColor = () =>
      page.evaluate(
        () =>
          document
            .querySelector('meta[name="theme-color"]')
            ?.getAttribute('content') ?? null,
      )

    // Dark: <html class="dark"> + a theme-color is set.
    await setTheme(page, 'dark')
    expect(await isDarkMode(page)).toBe(true)
    const darkColor = await metaColor()
    expect(darkColor, 'dark meta theme-color must be set').toBeTruthy()

    // Light: <html> drops `dark`, gains `light`, and the meta theme-color
    // REACTS (different container background than dark).
    await setTheme(page, 'light')
    expect(await isDarkMode(page)).toBe(false)
    expect(
      await page.evaluate(() =>
        document.documentElement.classList.contains('light'),
      ),
    ).toBe(true)
    const lightColor = await metaColor()
    expect(lightColor, 'light meta theme-color must be set').toBeTruthy()
    expect(
      lightColor,
      'meta theme-color must differ between light and dark',
    ).not.toBe(darkColor)
  })
})
