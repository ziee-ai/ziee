import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — cross-device sync of web search settings.
 *
 * The backend emits `SyncEntity::WebSearchSettings` on update
 * (web_search/handlers.rs); the WebSearchAdmin store subscribes to
 * `sync:web_search_settings` and reloads, and WebSearchGlobalSection re-seeds
 * the form (when not dirty). So a change on device A reflects on device B's
 * /settings/web-search WITHOUT reload. Admin↔admin, --workers=1.
 */

async function gotoWebSearch(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/web-search`)
  await expect(
    page.getByRole('heading', { name: 'Web Search' }),
  ).toBeVisible({ timeout: 30000 })
}

function enableSwitch(page: import('@playwright/test').Page) {
  return page
    .locator('.ant-form-item:has-text("Enable web search")')
    .getByRole('switch')
}

test.describe('Realtime sync — web search settings (cross-window)', () => {
  test('toggling enabled on device A reflects on device B live', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoWebSearch(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Read the current enabled state.
    const cur = await (
      await fetch(`${apiURL}/api/web-search/settings`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()
    const next = !cur.enabled

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoWebSearch(pageB, baseURL)
      // Device B's switch starts at the current state.
      const before = enableSwitch(pageB)
      await expect(before).toHaveAttribute(
        'aria-checked',
        String(cur.enabled),
        { timeout: 15000 },
      )

      // Flip `enabled` on device A via the REST API.
      const put = await fetch(`${apiURL}/api/web-search/settings`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ enabled: next }),
      })
      expect(put.ok).toBeTruthy()

      // Device B's switch updates live via the sync→reload→re-seed path.
      await expect(enableSwitch(pageB)).toHaveAttribute(
        'aria-checked',
        String(next),
        { timeout: 15000 },
      )
    } finally {
      await ctxB.close()
    }
  })
})
