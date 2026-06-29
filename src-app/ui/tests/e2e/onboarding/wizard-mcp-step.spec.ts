import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding wizard "MCP Servers" step admin controls
 * (audit id 8b02a1aeb8ce). The existing onboarding-wizard.spec.ts clicks
 * straight through the step without exercising its real actions. This reaches
 * the step as a user WITH the admin MCP perms and toggles a system server's
 * enable switch (McpServersStep.tsx toggleSystemServer), proving the control
 * works in-wizard.
 */

async function mcpAdminUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
    'mcp_servers_admin::read',
    'mcp_servers_admin::edit',
  ])
  return { username, adminToken }
}

async function seedSystemServer(apiURL: string, adminToken: string, name: string) {
  const res = await fetch(`${apiURL}/api/mcp/system-servers`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
    body: JSON.stringify({
      name,
      display_name: name,
      description: 'Wizard step test server',
      enabled: true,
      transport_type: 'http',
      url: 'http://127.0.0.1:9/mcp',
      usage_mode: 'auto',
    }),
  })
  if (!res.ok) throw new Error(`seed system server failed: ${res.status} ${await res.text()}`)
}

test.describe('Onboarding wizard — MCP Servers step', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('admin can toggle a system server on the MCP Servers step', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const serverName = `wizard-mcp-${Date.now()}`
    const { username, adminToken } = await mcpAdminUser(apiURL, 'wizmcp')
    await seedSystemServer(apiURL, adminToken, serverName)

    await loginExpectingOnboarding(page, baseURL, username, 'password123')
    await expect(page).toHaveURL(new RegExp('/onboarding'))

    // Welcome → AI Providers → MCP Servers.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()

    // The seeded system server row renders with an enabled toggle.
    await expect(page.getByTestId(/^onboarding-mcp-system-server-row-/).filter({ hasText: serverName })).toBeVisible({ timeout: 15000 })
    const row = page
      .getByTestId(/^onboarding-mcp-system-server-row-/)
      .filter({ hasText: serverName })
      .first()
    const toggle = row.getByTestId(/^onboarding-mcp-system-server-switch-/)
    await expect(toggle).toBeChecked()

    // Toggle it off → the wizard step's toggleSystemServer flips the control.
    await toggle.click()
    await expect(toggle).not.toBeChecked()
  })
})
