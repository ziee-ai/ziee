import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { setTheme } from '../../utils/theme'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { createAdminViaSetup } from './helpers/form-helpers'
import { logoutAndGoToAuth } from './helpers/navigation-helpers'

// TEST-5 (covers ITEM-5): the backdrop edge color follows the theme. The
// `--auth-backdrop` token resolves to different values light↔dark, the iOS
// meta[theme-color] (driven from that token) differs light↔dark, and axe AA
// contrast passes over the backdrop in BOTH themes — on login AND setup.

async function metaThemeColor(page: Page): Promise<string | null> {
  return page.evaluate(
    () =>
      document
        .querySelector('meta[name="theme-color"]')
        ?.getAttribute('content') ?? null,
  )
}

async function authBackdropVar(page: Page): Promise<string> {
  return page.evaluate(() =>
    getComputedStyle(document.documentElement)
      .getPropertyValue('--auth-backdrop')
      .trim(),
  )
}

async function readBothThemes(page: Page, waitSelector: string) {
  await setTheme(page, 'light')
  await byTestId(page, waitSelector).waitFor({ timeout: 30000 })
  await expect.poll(() => metaThemeColor(page)).not.toBeNull()
  const varLight = await authBackdropVar(page)
  const metaLight = await metaThemeColor(page)
  await assertNoAccessibilityViolations(page)

  await setTheme(page, 'dark')
  await byTestId(page, waitSelector).waitFor({ timeout: 30000 })
  await expect.poll(() => metaThemeColor(page)).not.toBeNull()
  const varDark = await authBackdropVar(page)
  const metaDark = await metaThemeColor(page)
  await assertNoAccessibilityViolations(page)

  return { varLight, varDark, metaLight, metaDark }
}

test.describe('Auth backdrop follows the theme', () => {
  test('login: --auth-backdrop + meta[theme-color] differ light↔dark, AA passes both', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await createAdminViaSetup(page, baseURL)
    await logoutAndGoToAuth(page, baseURL)

    const r = await readBothThemes(page, 'auth-login-username')
    expect(r.varLight).not.toBe('')
    expect(r.varDark).not.toBe(r.varLight)
    expect(r.metaLight).not.toBeNull()
    expect(r.metaDark).not.toBe(r.metaLight)
  })

  test('setup: --auth-backdrop + meta[theme-color] differ light↔dark, AA passes both', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    const r = await readBothThemes(page, 'app-setup-username-input')
    expect(r.varLight).not.toBe('')
    expect(r.varDark).not.toBe(r.varLight)
    expect(r.metaLight).not.toBeNull()
    expect(r.metaDark).not.toBe(r.metaLight)
  })
})
