import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSettingsPage, waitForSettingsPageLoad } from './helpers/navigation-helpers'

/**
 * E2E — the theme preference persists across browser SESSIONS (audit 5978d294cf2a).
 *
 * `ConfigClient.store.ts` persists `themePreference` via zustand's `persist`
 * middleware into `localStorage['config-client-storage']`. The existing
 * `theme-reflects.spec.ts` only proves a single in-context `page.reload()`
 * re-applies it. The genuinely-uncovered behavior is cross-SESSION restore:
 * close the browser and reopen it (a brand-new context with fresh in-memory
 * JS state but the persisted profile) — the theme must come back up from the
 * persisted preference alone, without ever re-visiting settings.
 *
 * This drives the real `#theme-form` selector to set Dark, captures the
 * persisted `storageState` (the faithful "reopened browser" snapshot), opens
 * a NEW context from it, and asserts `<html>` carries `dark` on first paint of
 * a non-settings route — and, as a control, that a fresh context WITHOUT the
 * persisted theme does NOT come up dark. Nothing is mocked.
 */

async function pickTheme(page: import('@playwright/test').Page, title: 'Dark' | 'Light') {
  await page.locator('#theme-form [aria-label="Theme"]').first().click()
  await page
    .getByRole('listbox')
    .or(page.locator('.ant-select-dropdown'))
    .first()
    .waitFor({ state: 'visible' })
  const option = page.getByTitle(title, { exact: true })
  await option.waitFor({ state: 'visible', timeout: 5000 })
  await option.click()
  await page.waitForTimeout(400) // let the ThemeProvider effect run + persist
}

function isDark(page: import('@playwright/test').Page) {
  return page.evaluate(() => document.documentElement.classList.contains('dark'))
}

test.describe('Settings — theme persists across browser sessions', () => {
  test('a fresh session restores the persisted Dark theme without re-selecting it', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToSettingsPage(page, baseURL, 'general')
    await waitForSettingsPageLoad(page, 'General')

    // Set Dark via the real control and confirm it applied + was persisted.
    await pickTheme(page, 'Dark')
    expect(await isDark(page), 'dark applied in the original session').toBe(true)

    const persisted = await page.evaluate(() =>
      window.localStorage.getItem('config-client-storage'),
    )
    expect(persisted, 'theme preference is persisted to localStorage').toBeTruthy()
    expect(
      JSON.parse(persisted!).state.themePreference,
      'persisted preference is dark',
    ).toBe('dark')

    // Snapshot the persisted profile (cookies + localStorage) — this is the
    // "reopen the browser" state that a brand-new session would start from.
    const sessionState = await page.context().storageState()

    // ---- New browser SESSION: fresh context, no shared in-memory state. ----
    const restoredCtx = await browser.newContext({ storageState: sessionState })
    try {
      const restoredPage = await restoredCtx.newPage()
      // Land on a NON-settings route so the theme can only come from the
      // persisted preference (not from anything the settings page does).
      await restoredPage.goto(`${baseURL}/`, { waitUntil: 'domcontentloaded' })
      await expect
        .poll(() => isDark(restoredPage), {
          message: 'persisted Dark theme is re-applied in a fresh session',
          timeout: 10_000,
        })
        .toBe(true)
      await restoredPage.close()
    } finally {
      await restoredCtx.close()
    }

    // ---- Control: a fresh session WITHOUT the persisted theme is not dark. ----
    // Strip only the theme key from the snapshot, keep auth so it still loads.
    const noThemeState = {
      ...sessionState,
      origins: sessionState.origins.map(o => ({
        ...o,
        localStorage: o.localStorage.filter(
          e => e.name !== 'config-client-storage',
        ),
      })),
    }
    const controlCtx = await browser.newContext({ storageState: noThemeState })
    try {
      const controlPage = await controlCtx.newPage()
      await controlPage.goto(`${baseURL}/`, { waitUntil: 'domcontentloaded' })
      await controlPage.waitForTimeout(800) // give ThemeProvider time to settle
      expect(
        await isDark(controlPage),
        'without the persisted preference the session is not dark',
      ).toBe(false)
      await controlPage.close()
    } finally {
      await controlCtx.close()
    }
  })
})
