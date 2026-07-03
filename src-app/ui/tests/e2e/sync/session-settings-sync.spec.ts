import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — cross-device sync of the admin session settings.
 *
 * The backend emits `SyncEntity::SessionSettings` on update
 * (auth/session_settings.rs); the SessionSettings store subscribes to
 * `sync:session_settings` and reloads, and SessionSettingsPage re-seeds
 * the form (when not dirty). So a change on device A reflects on device
 * B's /settings/sessions WITHOUT reload. Admin↔admin, --workers=1.
 */

async function gotoSessions(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/sessions`)
  await expect(byTestId(page, 'session-settings-card')).toBeVisible({
    timeout: 30000,
  })
}

test.describe('Realtime sync — session settings (cross-window)', () => {
  test('changing session length on device A reflects on device B live', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoSessions(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Read the current value + pick a distinct target.
    const cur = await (
      await fetch(`${apiURL}/api/auth/session-settings`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()
    const next = cur.refresh_token_expiry_days === 21 ? 14 : 21

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoSessions(pageB, baseURL)
      // Device B's field starts at the current value.
      await expect(
        byTestId(pageB, 'session-settings-session-days'),
      ).toHaveValue(new RegExp(String(cur.refresh_token_expiry_days)), {
        timeout: 15000,
      })

      // Change it on device A via the REST API.
      const put = await fetch(`${apiURL}/api/auth/session-settings`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ refresh_token_expiry_days: next }),
      })
      expect(put.ok).toBeTruthy()

      // Device B's field updates live via the sync→reload→re-seed path.
      await expect(
        byTestId(pageB, 'session-settings-session-days'),
      ).toHaveValue(new RegExp(String(next)), { timeout: 15000 })
    } finally {
      await ctxB.close()
    }
  })
})
