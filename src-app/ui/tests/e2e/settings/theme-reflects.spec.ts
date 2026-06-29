import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad, selectThemeOption } from './helpers/navigation-helpers'

/**
 * E2E — a theme change actually REFLECTS in the rendered app shell.
 *
 * Audit gap (all-720a77dd332a): the existing `should change theme using
 * selector` test only asserts that `<html>` gains the `dark` class one way
 * (light → dark). It never asserts the reverse direction, never asserts a
 * *visible* shell reflection, and never asserts persistence across a reload.
 *
 * `ThemeProvider` (components/ThemeProvider/ThemeProvider.tsx) applies the
 * theme by (a) toggling the `dark`/`light` class on `document.documentElement`
 * and (b) writing the active theme's `colorBgContainer` token into the
 * `<meta name="theme-color">` content — a concrete, theme-specific shell
 * signal. This spec drives the real theme Select (`settingsgen-theme-select`) and asserts BOTH
 * signals flip when switching dark↔light, that they revert, and that the
 * choice survives a page reload. Nothing is mocked.
 */

async function readThemeSignals(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const root = document.documentElement
    const meta = document.querySelector('meta[name="theme-color"]')
    return {
      isDark: root.classList.contains('dark'),
      isLight: root.classList.contains('light'),
      themeColor: meta?.getAttribute('content') ?? null,
    }
  })
}

async function pickTheme(page: import('@playwright/test').Page, title: 'Dark' | 'Light') {
  await selectThemeOption(page, title.toLowerCase() as 'dark' | 'light')
  await page.waitForTimeout(400) // let the ThemeProvider effect run
}

test.describe('Settings — theme reflects in the app shell', () => {
  test('switching dark↔light flips the html class + theme-color meta and persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Start from a known LIGHT baseline so the assertions are unambiguous.
    await pickTheme(page, 'Light')
    const light = await readThemeSignals(page)
    expect(light.isLight, 'html should carry the light class').toBe(true)
    expect(light.isDark).toBe(false)
    expect(light.themeColor, 'theme-color meta is set in light mode').toBeTruthy()

    // Switch to DARK — both signals must flip in the rendered shell.
    await pickTheme(page, 'Dark')
    const dark = await readThemeSignals(page)
    expect(dark.isDark, 'html should carry the dark class').toBe(true)
    expect(dark.isLight).toBe(false)
    expect(dark.themeColor, 'theme-color meta is set in dark mode').toBeTruthy()
    expect(
      dark.themeColor,
      'dark shell background differs from light (visible reflection)',
    ).not.toBe(light.themeColor)

    // Revert to LIGHT — the shell must revert too (not a one-way latch).
    await pickTheme(page, 'Light')
    const reverted = await readThemeSignals(page)
    expect(reverted.isLight).toBe(true)
    expect(reverted.isDark).toBe(false)
    expect(reverted.themeColor).toBe(light.themeColor)

    // Persistence: set DARK, reload, and the rendered shell stays dark
    // (the preference is restored from localStorage on mount).
    await pickTheme(page, 'Dark')
    await page.reload({ waitUntil: 'domcontentloaded' })
    await waitForSettingsPageLoad(page, 'General')
    const afterReload = await readThemeSignals(page)
    expect(afterReload.isDark, 'dark theme survives a reload').toBe(true)
    expect(afterReload.themeColor).toBe(dark.themeColor)
  })
})
