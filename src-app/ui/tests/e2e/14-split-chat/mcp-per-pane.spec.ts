import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane MCP selection (TEST-53, ITEM-33). MCP server selection
 * is conversation-keyed (`conversationConfigs` / `getSelectedServersConfigFor`), so
 * enabling a server for pane B's conversation shows its chip in pane B's composer
 * ONLY — the last-loaded pane never hijacks the other. (The wrong-pane tool-
 * approval routing itself is proven deterministically at the unit level by
 * `approvalRouting.test.ts`, since an e2e approval needs a real MCP tool call with
 * an approval-gated tool.) No LLM.
 */
test.describe('Split chat — per-pane MCP selection', () => {
  const mkConv = async (
    page: import('@playwright/test').Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  test('enabling a server for pane B shows its chip in pane B only, not pane A', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    // Seed an enabled http MCP server (http has no sandbox requirement in E2E).
    const stamp = Date.now()
    const srvRes = await page.request.post(`${apiURL}/api/mcp/servers`, {
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      data: {
        name: `e2e-split-mcp-${stamp}`,
        display_name: `Split MCP ${stamp}`,
        transport_type: 'http',
        url: 'https://e2e-split-mcp.example.invalid/mcp',
        enabled: true,
      },
    })
    expect(srvRes.ok()).toBeTruthy()
    const serverId = (await srvRes.json()).id as string

    const convA = await mkConv(page, apiURL, token, 'MCP Pane Alpha')
    const convB = await mkConv(page, apiURL, token, 'MCP Pane Bravo')

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })

    // Focus pane B and open its MCP config modal (scoped to pane B's composer).
    await pane1.click()
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })

    // Enable the server for pane B's conversation, then close.
    const toggle = byTestId(page, `mcp-config-server-switch-${serverId}`)
    if ((await toggle.getAttribute('aria-checked')) !== 'true') await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
    await byTestId(page, 'mcp-config-close-btn').click()

    // Pane B's composer now shows the server chip; pane A's does NOT — the
    // selection is per-conversation, not a shared global.
    await expect(pane1.getByTestId(`mcp-chip-${serverId}`)).toBeVisible({ timeout: 10000 })
    await expect(pane0.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0)
  })
})
