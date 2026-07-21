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
 * TEST-32 (A10 [negative-perm]) — the agent admin-settings surface
 * (`/settings/agent`, gated on `agent::settings::read`) is ABSENT for a user who
 * lacks the permission, at every gating layer:
 *   - slot: the `settingsAdminPages` "Agent" nav item is filtered out.
 *   - route: navigating directly to `/settings/agent` does NOT render the page.
 * The admin (who holds `*`) sees both — the positive control.
 */
test.describe('Agent settings — permission gating (negative-perm)', () => {
  test('admin sees the Agent settings section; a user lacking agent::settings::read does not', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // --- Admin: the Agent admin section is present + the page renders. ---
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await page.goto(`${baseURL}/settings/profile`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-agent'),
    ).toBeVisible({ timeout: 30000 })

    await page.goto(`${baseURL}/settings/agent`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'agent-settings-card')).toBeVisible({
      timeout: 30000,
    })

    // --- Regular user WITHOUT agent::settings::read: absent everywhere. ---
    const uname = `agentnoperm_${Date.now().toString(36)}`
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

    // Slot layer: the settings menu renders, but the Agent item is filtered out.
    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    await expect(
      byTestId(page, 'settings-nav-menu-item-agent'),
    ).toHaveCount(0)

    // Route layer: a direct hit on /settings/agent must NOT render the agent page
    // (the route guard redirects / denies — the AgentSettingsPage's own testid
    // never appears for this user).
    await page.goto(`${baseURL}/settings/agent`)
    await expect(
      byTestId(page, 'agent-settings-card'),
    ).toHaveCount(0, { timeout: 15000 })
  })
})
