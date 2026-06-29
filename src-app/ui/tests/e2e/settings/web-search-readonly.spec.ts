import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the read-only permission path on the Web Search admin settings page.
 *
 * `WebSearchGlobalSection` (web-search/components/WebSearchGlobalSection.tsx:120-127)
 * renders a "Read-only view" Alert and disables the whole settings Form (and
 * the Save button) when the user lacks `web_search::admin::manage`. Every prior
 * web-search spec logs in as a full admin, so the `!canManage` branch was never
 * exercised. This creates a user holding ONLY `web_search::admin::read` (enough
 * to reach the route + load settings, but not to change them) and asserts the
 * read-only affordances. The positive control confirms a manage user is NOT
 * read-only — proving the gate actually gates.
 */

test.describe('Web search settings — read-only permission path', () => {
  test('a read-only user sees the read-only Alert and cannot edit', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const uname = `ws_ro_${tag}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit', 'web_search::admin::read'],
    )

    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await page.goto(`${baseURL}/settings/web-search`)

    // The read-only Alert renders (the !canManage branch).
    await expect(
      byTestId(page, 'websearch-global-readonly-alert'),
    ).toBeVisible({ timeout: 15000 })

    // The Save button is present but disabled (no manage permission).
    await expect(byTestId(page, 'websearch-global-save')).toBeDisabled({
      timeout: 10000,
    })
  })

  test('a manage user is NOT read-only (positive control)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/web-search`)

    // Admin holds `*`, so the read-only Alert must be absent and the
    // settings Card is rendered editable.
    await expect(byTestId(page, 'websearch-global-card')).toBeVisible({
      timeout: 15000,
    })
    await expect(
      byTestId(page, 'websearch-global-readonly-alert'),
    ).toHaveCount(0)
  })
})
