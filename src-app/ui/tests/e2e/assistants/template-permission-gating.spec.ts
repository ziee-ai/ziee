import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
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
    await expect(byTestId(page, 'template-assistants-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'template-assistants-create-btn')).toBeVisible()

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
    await expect(byTestId(page, 'template-assistants-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'template-assistants-create-btn')).toHaveCount(0)
  })

  test('user without template read is denied the templates page (route gate)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A non-admin lacking `assistant_templates::read` entirely. The route
    // (`/settings/assistant-templates`, permission: AssistantsTemplateRead)
    // must refuse access — SettingsPage renders an inline 403 "Not authorized"
    // panel rather than the templates surface (deep-link 403, not a silent
    // redirect). This is the access gate the create-button test never exercises
    // (that user holds template::read).
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const uname = `tmplnoread_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await page.goto(`${baseURL}/settings/assistant-templates`)

    // The inline 403 panel is shown...
    const forbidden = byTestId(page, 'settings-forbidden-result')
    await expect(forbidden).toBeVisible({ timeout: 30000 })
    await expect(forbidden).toContainText(/don't have permission to view/i)

    // ...and the templates surface itself never renders for this user.
    await expect(byTestId(page, 'template-assistants-card')).toHaveCount(0)
    await expect(byTestId(page, 'template-assistants-create-btn')).toHaveCount(0)
  })
})
