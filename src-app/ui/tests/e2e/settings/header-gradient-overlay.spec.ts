import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * App-shell header separation.
 *
 * The former soft-fade gradient strip below the header was replaced in the
 * chrome consistency sweep by a solid header bar with a bottom border
 * (`border-b border-border` on HeaderBarContainer's bar, testid
 * `app-header-bar`). This asserts the app shell still renders that separation
 * affordance below the header — the bar exists and carries a visible bottom
 * border — rather than a brittle screenshot.
 */
test.describe('App shell — header separation', () => {
  test('renders the header bar with a visible bottom border', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Any authenticated page carries the app-shell header.
    await page.goto(`${baseURL}/settings/general`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 15000,
    })

    const headerBar = byTestId(page, 'app-header-bar')
    await expect(headerBar).toBeVisible({ timeout: 10000 })

    const border = await headerBar.evaluate((el) => {
      const s = getComputedStyle(el)
      return {
        width: s.borderBottomWidth,
        style: s.borderBottomStyle,
      }
    })

    expect(border.style, 'header bar has a solid bottom border').toBe('solid')
    expect(
      parseFloat(border.width),
      'header bar bottom border has non-zero width',
    ).toBeGreaterThan(0)
  })
})
