import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * E2E — MULTI-server MCP handling in a single conversation.
 *
 * The existing chat-MCP specs (sampling / sandbox-plot) only ever register one
 * server. This seeds TWO user MCP servers and asserts both appear in the
 * conversation's MCP config modal and can both be enabled together — the
 * multi-server selection/orchestration surface single-server specs miss. (The
 * heavier cross-server real-LLM tool-call lives behind ANTHROPIC_API_KEY +
 * mock tool servers.)
 */

async function seedServer(apiURL: string, token: string, display: string) {
  const res = await fetch(`${apiURL}/api/mcp/servers`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({
      name: `${display.toLowerCase().replace(/\s+/g, '-')}-${Date.now()}`,
      display_name: display,
      transport_type: 'stdio',
      command: 'node',
      args: ['server.js'],
      enabled: true,
    }),
  })
  if (!res.ok) throw new Error(`seed server failed: ${res.status}`)
}

test.describe('Chat — multi-server MCP config', () => {
  test('two MCP servers both appear and can be enabled in one conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const srvA = `E2E MultiSrv A ${Date.now()}`
    const srvB = `E2E MultiSrv B ${Date.now()}`
    await seedServer(apiURL, token, srvA)
    await seedServer(apiURL, token, srvB)

    await goToNewChatPage(page, baseURL)
    await byTestId(page, 'chat-input-add-btn').first().click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })

    // BOTH servers are listed in the modal.
    const headerA = page.getByTestId(/^mcp-config-server-row-/).filter({ hasText: srvA })
    const headerB = page.getByTestId(/^mcp-config-server-row-/).filter({ hasText: srvB })
    await expect(headerA).toBeVisible({ timeout: 10000 })
    await expect(headerB).toBeVisible()

    // Enable BOTH server toggles → both become selected.
    const toggleA = headerA.getByTestId(/^mcp-config-server-switch-/)
    const toggleB = headerB.getByTestId(/^mcp-config-server-switch-/)
    if ((await toggleA.getAttribute('aria-checked')) === 'false') await toggleA.click()
    if ((await toggleB.getAttribute('aria-checked')) === 'false') await toggleB.click()
    await expect(toggleA).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
    await expect(toggleB).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
  })
})
