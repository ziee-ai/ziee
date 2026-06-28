import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'

/**
 * E2E — permission gating on the Assistant Templates admin surface
 * (AssistantsSettings.tsx — the `<Can permission={AssistantsTemplateCreate}>`
 * around the create affordance + the canEdit/canDelete gates).
 *
 * Audit gap: a user with template READ but no CREATE can view the page but
 * must NOT see the create affordance. The admin (wildcard) does.
 */

test.describe('Assistant Templates — permission gating', () => {
  test('read-only template user sees the page but not the create button', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Admin: the "Create assistant" affordance is present.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await page.goto(`${baseURL}/settings/assistant-templates`)
    await expect(
      page.getByRole('heading', { name: 'Assistant Templates' }),
    ).toBeVisible({ timeout: 30000 })
    await expect(
      page.getByRole('button', { name: 'Create assistant' }),
    ).toBeVisible()

    // Read-only template user: page renders, create button is gated out.
    const uname = `tmplro_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit', 'assistant_templates::read'],
    )
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await page.goto(`${baseURL}/settings/assistant-templates`)
    await expect(
      page.getByRole('heading', { name: 'Assistant Templates' }),
    ).toBeVisible({ timeout: 30000 })
    await expect(
      page.getByRole('button', { name: 'Create assistant' }),
    ).toHaveCount(0)
  })
})
