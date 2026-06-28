import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — MCP server installation during onboarding (McpServersStep.tsx:
 * "Install from Hub" selection → selectedMcpServerIds → installSelectedMcpServers
 * on finish; FinishStep.tsx:18-20 summary). The wizard happy-path clicks through
 * without selecting any server; the select-to-install path was untested.
 */

async function freshHubUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  // hub::mcp_servers::create flips canInstallFromHub → the "Install from Hub"
  // section renders for this onboarding user.
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
    'hub::mcp_servers::create',
    'mcp_servers::read',
  ])
  return username
}

test.describe('Onboarding — MCP server install', () => {
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

  test('selecting a hub MCP server marks it for installation on Finish', async ({
  test('toggling a system MCP server on then Next advances the wizard', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await freshHubUser(apiURL, 'mcpinst')
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

    // Select the first hub server in the "Install from Hub" section.
    await expect(page.getByText('Install from Hub')).toBeVisible({ timeout: 15000 })
    await page.getByRole('checkbox').first().check()

    // MCP Servers → Memory → Finish.
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByRole('heading', { name: 'Persistent Memory' })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()

    // FinishStep reflects the selected server (not "No MCP servers selected").
    await expect(page.getByRole('heading', { name: /all set/i })).toBeVisible({ timeout: 10000 })
    await expect(page.getByText(/MCP server.*selected for installation/i)).toBeVisible()
    await expect(page.getByText(/No MCP servers selected/i)).toHaveCount(0)
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
