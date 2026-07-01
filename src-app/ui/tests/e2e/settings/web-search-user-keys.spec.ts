import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import {
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — per-user web-search provider keys (path: /settings/web-search-keys).
 *
 * Drives the REAL backend (no page.route mocking): an admin sets the deployment
 * Brave key via the API, a regular user (web_search::use via the default Users
 * group) enters their OWN key in the UI, and we assert the masked state, that
 * the raw key is never shown, that it persists across reload (mount-time fetch,
 * not event-only), and that clearing it falls back to the shared-key state.
 */

const KEYS_PATH = '/settings/web-search-keys'

async function gotoKeys(page: Page, baseURL: string) {
  await page.goto(`${baseURL}${KEYS_PATH}`)
  await expect(byTestId(page, 'websearch-user-keys-card')).toBeVisible({
    timeout: 30000,
  })
}

async function setDeploymentBraveKey(apiURL: string, adminToken: string, key: string) {
  const res = await fetch(`${apiURL}/api/web-search/providers/brave`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify({ api_key: key }),
  })
  if (!res.ok) throw new Error(`set deployment key failed: ${res.status}`)
}

test.describe('Per-user web search keys', () => {
  test('user sets, sees masked, persists across reload, and clears their own key', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    // Deployment key present → the fallback "Using shared key" state is visible.
    await setDeploymentBraveKey(apiURL, adminToken, 'DEPLOYMENT-SHARED-KEY')

    const uname = `wsuk_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@e2e.test`, 'Passw0rd!')
    await login(page, baseURL, uname, 'Passw0rd!')

    await gotoKeys(page, baseURL)

    // Before setting a personal key: the shared-key fallback tag shows.
    await expect(byTestId(page, 'websearch-user-key-brave-status')).toContainText(
      'Using shared key',
    )

    // Enter a personal key and save.
    await byTestId(page, 'websearch-user-key-brave-input').fill('MY-PERSONAL-KEY')
    await byTestId(page, 'websearch-user-keys-save').click()

    // Masked status appears; the raw key is never shown on the page.
    const status = byTestId(page, 'websearch-user-key-brave-status')
    await expect(status).toContainText('Using your key', { timeout: 10000 })
    await expect(page.locator('body')).not.toContainText('MY-PERSONAL-KEY')

    // Persists across a full reload (mount-time fetch, not event-only).
    await page.reload()
    await expect(byTestId(page, 'websearch-user-keys-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'websearch-user-key-brave-status')).toContainText(
      'Using your key',
    )

    // Clear it → falls back to the shared-key state.
    await byTestId(page, 'websearch-user-key-brave-clear').click()
    await expect(byTestId(page, 'websearch-user-key-brave-status')).toContainText(
      'Using shared key',
      { timeout: 10000 },
    )
  })
})
