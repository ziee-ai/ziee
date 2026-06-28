import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * Onboarding "MCP Servers" step — toggling a system MCP server ON and advancing
 * runs the `registerBeforeNext` apply handler (`applyMcpServerChanges`), which
 * persists the toggle / installs selections. This focused spec exercises that
 * install-on-Next flow (the path that previously failed silently): a user with
 * MCP-admin permission sees the system-server toggles, enables one, clicks
 * Next, and the wizard advances to the next step — proving the apply handler
 * ran without surfacing an error.
 *
 * The generic step-walk is covered by onboarding-wizard.spec.ts; this adds the
 * MCP-install action the wizard walk skips (it just clicks Next with no toggle).
 */
test.describe('Onboarding - MCP server install on Next', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('toggling a system MCP server on then Next advances the wizard', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `mcp_onb_${Date.now().toString(36)}`
    // Needs MCP-admin perms so the system-server toggles render in the step.
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit', 'mcp_servers_admin::edit', 'mcp_servers_admin::read'],
    )

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers → MCP Servers.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByRole('heading', { name: 'MCP Servers' })).toBeVisible()

    // Enable the first system MCP server toggle (antd Switch → role="switch").
    const firstSwitch = page.getByRole('switch').first()
    await expect(firstSwitch).toBeVisible({ timeout: 15000 })
    if ((await firstSwitch.getAttribute('aria-checked')) !== 'true') {
      await firstSwitch.click()
    }
    await expect(firstSwitch).toHaveAttribute('aria-checked', 'true')

    // Next runs applyMcpServerChanges; the wizard must advance (no silent
    // failure stalling on the MCP step) to the Memory step.
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(
      page.getByRole('heading', { name: 'Persistent Memory' }),
    ).toBeVisible({ timeout: 15000 })
  })
})
