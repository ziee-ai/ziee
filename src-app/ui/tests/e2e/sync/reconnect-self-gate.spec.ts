import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'

// Network disconnect → reconnect → `sync:reconnect` store self-gating.
//
// The SyncClient streams over fetch; going offline drops the stream and the
// reconnect loop retries, emitting `sync:reconnect` once the network returns
// (SyncClient.ts). Per the no-403 reconnect rule (CLAUDE.md), every store's
// `sync:reconnect` handler must call hasPermissionNow(...) and skip its refetch
// when the user lacks the read perm — otherwise the reconnect would fire a
// burst of 403s for a permission-restricted user. This asserts that a minimal
// (profile-only) user survives an offline→online cycle WITHOUT any /api/* 403.
//
// Run with --workers=1 (shared backend + DB).

test.describe('Sync reconnect self-gating', () => {
  test.describe.configure({ retries: 2 })

  test('offline→online reconnect produces no 403 for a restricted user', async ({
    page,
    context,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A minimal user: profile-only, so most admin/entity read endpoints are
    // forbidden — exactly the case where a non-self-gated reconnect would 403.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const username = `recon_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await login(page, baseURL, username, 'password123')

    // Land on the app shell.
    await page.goto(`${baseURL}/`)
    await expect(
      byTestId(page, 'chat-history-new-chat-btn'),
    ).toBeVisible({ timeout: 20_000 })

    // Record any forbidden API response from here on.
    const forbidden: string[] = []
    page.on('response', resp => {
      if (resp.status() === 403 && resp.url().includes('/api/')) {
        forbidden.push(resp.url())
      }
    })

    // Drop the network, then restore it — forces a genuine SyncClient
    // reconnect, which emits `sync:reconnect` to every store.
    await context.setOffline(true)
    await page.waitForTimeout(1500)
    await context.setOffline(false)

    // Give the reconnect + the resulting store refetches time to run.
    await page.waitForTimeout(6000)

    // The app is still usable and NO store fired a forbidden refetch.
    await expect(
      byTestId(page, 'chat-history-new-chat-btn'),
    ).toBeVisible()
    expect(
      forbidden,
      `sync:reconnect must self-gate per-permission; got 403s: ${forbidden.join(', ')}`,
    ).toEqual([])
  })
})
