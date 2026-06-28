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
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('selecting a hub MCP server marks it for installation on Finish', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await freshHubUser(apiURL, 'mcpinst')
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
  })
})
