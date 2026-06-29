import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-fd85458224a6 — realtime sync for the web_search_settings
// singleton. On a settings change the backend publishes WebSearchSettings/Update
// to every web_search::admin::read holder so other devices' WebSearchAdmin store
// (subscribed to sync:web_search_settings) refetches WITHOUT a reload. REAL
// cross-device test against the live SSE channel (no API mock).
//
// Run with --workers=1.

async function gotoWebSearch(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/web-search`)
  await page.waitForLoadState('load')
  await expect(
    byTestId(page, 'websearch-global-card'),
  ).toBeVisible({ timeout: 30_000 })
}

function enabledSwitch(page: import('@playwright/test').Page) {
  return page.getByRole('switch').first()
}

test.describe('Realtime sync — web_search_settings', () => {
  test('toggling enabled on device A updates device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await gotoWebSearch(page, baseURL)

    const getRes = await page.request.get(`${baseURL}/api/web-search/settings`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    expect(getRes.ok()).toBeTruthy()
    const initialEnabled = Boolean((await getRes.json()).enabled)
    const target = !initialEnabled

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoWebSearch(pageB, baseURL)

      // Sanity: device B reflects the initial enabled state.
      if (initialEnabled) {
        await expect(enabledSwitch(pageB)).toBeChecked({ timeout: 15_000 })
      } else {
        await expect(enabledSwitch(pageB)).not.toBeChecked({ timeout: 15_000 })
      }

      // Device A flips `enabled` via REST → publishes WebSearchSettings/Update.
      const putRes = await page.request.put(
        `${baseURL}/api/web-search/settings`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { enabled: target },
        },
      )
      expect(
        putRes.ok(),
        `PUT settings failed: ${putRes.status()} ${await putRes.text()}`,
      ).toBeTruthy()

      // Device B's switch must reflect the new value WITHOUT a manual reload.
      if (target) {
        await expect(enabledSwitch(pageB)).toBeChecked({ timeout: 45_000 })
      } else {
        await expect(enabledSwitch(pageB)).not.toBeChecked({ timeout: 45_000 })
      }
    } finally {
      await ctxB.close()
    }
  })
})
