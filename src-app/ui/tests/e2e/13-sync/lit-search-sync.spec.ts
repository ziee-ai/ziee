import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// Realtime sync for the `lit_search_settings` singleton entity. When an admin
// changes a literature-search setting (e.g. the completeness-estimate toggle),
// the backend publishes `LitSearchSettings/Update` to every
// `lit_search::admin::read` holder so other devices' LitSearchAdmin store
// (subscribed to `sync:lit_search_settings`) refetches WITHOUT a manual reload.
//
// audit id 48e5c16ff7b3 — LitSearchSettings had no E2E sync coverage. This is a
// REAL cross-device test against the live backend SSE channel (no API mock):
// device A flips the setting via the authenticated REST endpoint, device B's
// settings page must reflect it without reloading.
//
// Run with --workers=1.

const SETTINGS_URL = '/settings/literature'

async function gotoLiterature(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}${SETTINGS_URL}`)
  await page.waitForLoadState('load')
  await expect(
    page.getByRole('heading', { name: 'Literature Search' }),
  ).toBeVisible({ timeout: 30_000 })
}

function completenessSwitch(page: import('@playwright/test').Page) {
  return page.getByRole('switch', { name: 'Show completeness estimate' })
}

test.describe('Realtime sync — lit_search_settings', () => {
  test('changing a setting on device A updates device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await gotoLiterature(page, baseURL)

    // Read the authoritative current settings from the API.
    const getRes = await page.request.get(
      `${baseURL}/api/lit-search/settings`,
      { headers: { Authorization: `Bearer ${adminToken}` } },
    )
    expect(getRes.ok()).toBeTruthy()
    const initial = await getRes.json()
    const initialCompleteness = Boolean(initial.completeness_estimate_enabled)
    const target = !initialCompleteness

    // Device B — second context for the SAME admin. Load + subscribe BEFORE
    // device A mutates so its LitSearchAdmin store is mounted.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoLiterature(pageB, baseURL)

      // Sanity: device B reflects the initial value.
      if (initialCompleteness) {
        await expect(completenessSwitch(pageB)).toBeChecked({ timeout: 15_000 })
      } else {
        await expect(completenessSwitch(pageB)).not.toBeChecked({
          timeout: 15_000,
        })
      }

      // Device A flips the completeness toggle via REST → publishes
      // LitSearchSettings/Update.
      const putRes = await page.request.put(
        `${baseURL}/api/lit-search/settings`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { completeness_estimate_enabled: target },
        },
      )
      const putBody = await putRes.text()
      expect(
        putRes.ok(),
        `PUT settings failed: ${putRes.status()} ${putBody}`,
      ).toBeTruthy()

      // Device B's switch must reflect the new value WITHOUT a manual reload —
      // only true if LitSearchSettings/Update was published, SSE delivered it,
      // and the store refetched + re-rendered.
      if (target) {
        await expect(completenessSwitch(pageB)).toBeChecked({ timeout: 45_000 })
      } else {
        await expect(completenessSwitch(pageB)).not.toBeChecked({
          timeout: 45_000,
        })
      }
    } finally {
      await ctxB.close()
    }
  })
})
