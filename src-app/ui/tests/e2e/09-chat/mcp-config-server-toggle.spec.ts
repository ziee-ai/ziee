import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * E2E — McpConfigModal server-level toggle.
 *
 * The modal groups tools by server and exposes a per-server Switch
 * ("enable all tools from server", McpConfigModal.tsx:292 `handleServerToggle`).
 * The existing mcp-config-modal.spec only covers save semantics. This seeds a
 * USER MCP server (so it appears in the modal) and drives the server Switch.
 */

test.describe('MCP Config Modal — server toggle', () => {
  test('toggling a server Switch selects it', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // A model so the chat composer is usable.
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    // Seed a user MCP server (stdio) so it shows in the modal's server list.
    const display = `E2E Toggle Srv ${Date.now()}`
    const res = await fetch(`${apiURL}/api/mcp/servers`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({
        name: `e2e-toggle-${Date.now()}`,
        display_name: display,
        transport_type: 'stdio',
        command: 'node',
        args: ['server.js'],
        enabled: true,
      }),
    })
    expect(res.ok).toBeTruthy()

    await goToNewChatPage(page, baseURL)

    // Open the MCP config modal.
    await page.getByRole('button', { name: 'Add attachment' }).first().click()
    await page.getByText('MCP tools & servers').first().click()
    await expect(
      page.locator('.ant-modal-title:has-text("MCP Configuration")'),
    ).toBeVisible({ timeout: 10000 })

    // The seeded server appears with a per-server Switch in its collapse header.
    const header = page
      .locator('.ant-collapse-item')
      .filter({ hasText: display })
    await expect(header).toBeVisible({ timeout: 10000 })
    const toggle = header.locator('.ant-switch').first()

    // Toggle the server on → the Switch becomes checked (server selected).
    const before = await toggle.getAttribute('aria-checked')
    await toggle.click()
    await expect(toggle).toHaveAttribute(
      'aria-checked',
      before === 'true' ? 'false' : 'true',
      { timeout: 5000 },
    )
  })
})
