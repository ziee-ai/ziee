import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — realtime sync of the `AuthProvider` entity (perm-audience:
 * auth_providers::read). The admin handlers emit `SyncEntity::AuthProvider` on
 * create/update/delete/auto-disable (`auth/handlers.rs:1769,1852,1925,1987`).
 * No 13-sync spec covered it. Cross-window admin↔admin: a mutation on device A
 * reflects on device B's /settings/auth-providers list WITHOUT reload.
 *
 * Run with --workers=1.
 */

const API = '/api/admin/auth-providers'

async function gotoAuthProviders(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/auth-providers`)
  await page.waitForLoadState('domcontentloaded')
}

test.describe('Realtime sync — auth providers (cross-window)', () => {
  test('create then delete on device A reflect on device B', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoAuthProviders(page, baseURL)
    const token = await getAdminToken(apiURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoAuthProviders(pageB, baseURL)

      // Create a minimal OIDC provider on A (via API).
      const name = `xsync-oidc-${Date.now()}`
      const res = await fetch(`${apiURL}${API}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ name, provider_type: 'oidc', config: {} }),
      })
      if (!res.ok) throw new Error(`create auth provider failed: ${res.status}`)
      const id = (await res.json()).provider.id as string

      // Both windows list it live (row renders a "Toggle <name>" switch).
      await expect(
        page.getByRole('switch', { name: `Toggle ${name}` }),
      ).toBeVisible({ timeout: 15_000 })
      await expect(
        pageB.getByRole('switch', { name: `Toggle ${name}` }),
      ).toBeVisible({ timeout: 15_000 })

      // Delete on A → device B drops the row live.
      const del = await fetch(`${apiURL}${API}/${id}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      })
      expect(del.ok || del.status === 204).toBeTruthy()
      await expect(
        pageB.getByRole('switch', { name: `Toggle ${name}` }),
      ).toHaveCount(0, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})
