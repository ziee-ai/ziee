import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { getAdminToken, createTestUser, login } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — cross-device sync of a user's OWN web-search key.
 *
 * The backend emits owner-scoped `SyncEntity::WebSearchUserKey` on set/clear; the
 * WebSearchUserKeys store subscribes to `sync:web_search_user_key` and refetches.
 * So the same user saving on device A reflects on their device B WITHOUT reload.
 * A second user is the negative control (owner-scope isolation). --workers=1.
 */

const KEYS_PATH = '/settings/web-search-keys'

async function gotoKeys(page: Page, baseURL: string) {
  await page.goto(`${baseURL}${KEYS_PATH}`)
  await expect(byTestId(page, 'websearch-user-keys-card')).toBeVisible({
    timeout: 30000,
  })
}

test.describe('Realtime sync — per-user web search key', () => {
  test('saving on device A reflects on device B live; a second user is unaffected', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const uname = `wsuksync_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@e2e.test`, 'Passw0rd!')
    const other = `wsukother_${Date.now()}`
    await createTestUser(apiURL, adminToken, other, `${other}@e2e.test`, 'Passw0rd!')

    // Device A + device B are the SAME user in two contexts.
    await login(page, baseURL, uname, 'Passw0rd!')
    await gotoKeys(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    // The negative-control second user.
    const ctxC = await browser.newContext()
    const pageC = await ctxC.newPage()
    try {
      await login(pageB, baseURL, uname, 'Passw0rd!')
      await gotoKeys(pageB, baseURL)
      await login(pageC, baseURL, other, 'Passw0rd!')
      await gotoKeys(pageC, baseURL)

      // Device A saves a personal key.
      await byTestId(page, 'websearch-user-key-brave-input').fill('SYNC-KEY-A')
      await byTestId(page, 'websearch-user-keys-save').click()
      await expect(byTestId(page, 'websearch-user-key-brave-status')).toContainText(
        'Using your key',
        { timeout: 10000 },
      )

      // Device B reflects it live — NO reload.
      await expect(byTestId(pageB, 'websearch-user-key-brave-status')).toContainText(
        'Using your key',
        { timeout: 15000 },
      )

      // The other user is unaffected (owner-scope isolation).
      await expect(
        byTestId(pageC, 'websearch-user-key-brave-status'),
      ).not.toContainText('Using your key')
    } finally {
      await ctxB.close()
      await ctxC.close()
    }
  })
})
