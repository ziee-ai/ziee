import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import {
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — per-user lit-search connector keys (path: /settings/literature-keys).
 *
 * Drives the REAL backend: a regular user (lit_search::use via the default Users
 * group) enters their OWN CORE key in the UI, and we assert the masked state,
 * that the raw key is never shown, that it persists across reload, and clearing.
 */

const KEYS_PATH = '/settings/literature-keys'

async function gotoKeys(page: Page, baseURL: string) {
  await page.goto(`${baseURL}${KEYS_PATH}`)
  await expect(byTestId(page, 'litsearch-user-keys-card')).toBeVisible({
    timeout: 30000,
  })
}

test.describe('Per-user literature keys', () => {
  test('user sets, sees masked, persists across reload, and clears their own key', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const uname = `lsuk_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@e2e.test`, 'Passw0rd!')
    await login(page, baseURL, uname, 'Passw0rd!')

    await gotoKeys(page, baseURL)

    // CORE is a key-accepting connector → its row is present.
    await expect(byTestId(page, 'litsearch-user-key-core-input')).toBeVisible()

    await byTestId(page, 'litsearch-user-key-core-input').fill('MY-CORE-KEY')
    await byTestId(page, 'litsearch-user-keys-save').click()

    await expect(byTestId(page, 'litsearch-user-key-core-status')).toContainText(
      'Using your key',
      { timeout: 10000 },
    )
    await expect(page.locator('body')).not.toContainText('MY-CORE-KEY')

    await page.reload()
    await expect(byTestId(page, 'litsearch-user-keys-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'litsearch-user-key-core-status')).toContainText(
      'Using your key',
    )

    await byTestId(page, 'litsearch-user-key-core-clear').click()
    await expect(byTestId(page, 'litsearch-user-key-core-input')).toBeVisible({
      timeout: 10000,
    })
    await expect(byTestId(page, 'litsearch-user-key-core-status')).not.toContainText(
      'Using your key',
    )
  })
})
