import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { getAdminToken, createTestUser, login } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — cross-device sync of a user's OWN lit-search connector key.
 *
 * The backend emits owner-scoped `SyncEntity::LitSearchUserKey` on set/clear; the
 * LitSearchUserKeys store subscribes to `sync:lit_search_user_key` and refetches.
 * Same user on device A → device B reflects live; a second user is unaffected.
 * --workers=1.
 */

const KEYS_PATH = '/settings/literature-keys'

async function gotoKeys(page: Page, baseURL: string) {
  await page.goto(`${baseURL}${KEYS_PATH}`)
  await expect(byTestId(page, 'litsearch-user-keys-card')).toBeVisible({
    timeout: 30000,
  })
}

test.describe('Realtime sync — per-user literature key', () => {
  test('saving on device A reflects on device B live; a second user is unaffected', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const uname = `lsuksync_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@e2e.test`, 'Passw0rd!')
    const other = `lsukother_${Date.now()}`
    await createTestUser(apiURL, adminToken, other, `${other}@e2e.test`, 'Passw0rd!')

    await login(page, baseURL, uname, 'Passw0rd!')
    await gotoKeys(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    const ctxC = await browser.newContext()
    const pageC = await ctxC.newPage()
    try {
      await login(pageB, baseURL, uname, 'Passw0rd!')
      await gotoKeys(pageB, baseURL)
      await login(pageC, baseURL, other, 'Passw0rd!')
      await gotoKeys(pageC, baseURL)

      await byTestId(page, 'litsearch-user-key-core-input').fill('SYNC-CORE-A')
      await byTestId(page, 'litsearch-user-keys-save').click()
      await expect(byTestId(page, 'litsearch-user-key-core-status')).toContainText(
        'Using your key',
        { timeout: 10000 },
      )

      await expect(byTestId(pageB, 'litsearch-user-key-core-status')).toContainText(
        'Using your key',
        { timeout: 15000 },
      )

      await expect(
        byTestId(pageC, 'litsearch-user-key-core-status'),
      ).not.toContainText('Using your key')
    } finally {
      await ctxB.close()
      await ctxC.close()
    }
  })
})
