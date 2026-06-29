import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the Installed Hub tab renders ACTUAL tracked installs, not just the
 * empty state (the only thing 07-hub-version-activation covered). Install a hub
 * assistant via API, then assert the Installed tab's Assistants category shows
 * it (empty hint gone, the assistant appears).
 */

test.describe('Hub — Installed tab shows tracked installs', () => {
  test('an installed hub assistant appears in the Installed tab', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Pick a real seed-catalog assistant and install it for this user.
    const listing = (await (
      await fetch(`${apiURL}/api/hub/assistants?lang=en`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()) as Array<{ name: string; title?: string }>
    const hub = listing[0]
    const inst = await fetch(`${apiURL}/api/hub/assistants/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ hub_id: hub.name, enabled: true }),
    })
    expect(inst.ok).toBeTruthy()
    const assistant = (await inst.json()).assistant as { name: string }

    await page.goto(`${baseURL}/hub/installed`)
    await expect(page).toHaveURL(/\/hub\/installed/)

    // The Assistants category no longer shows its empty hint, and the installed
    // assistant's name is listed in a tracked row. (`assistant.name` is dynamic
    // data this test created, so filtering a row by it is allowed.)
    await expect(
      page.getByTestId('hub-installed-empty-assistant'),
    ).toHaveCount(0, { timeout: 15000 })
    await expect(
      page
        .getByTestId(/^hub-installed-row-/)
        .filter({ hasText: assistant.name })
        .first(),
    ).toBeVisible({ timeout: 15000 })
  })
})
