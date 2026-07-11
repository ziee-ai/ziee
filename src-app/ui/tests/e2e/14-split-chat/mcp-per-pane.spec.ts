import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane MCP surface (TEST-53, ITEM-33). The MCP config/menu
 * SURFACE is reachable per-pane (each split pane's composer carries its own `+`
 * tools button; opening the config modal from a pane and enabling a server applies
 * it). NOTE (DRIFT-2.11): the composer STATUS chip reads the GLOBAL single-active
 * `McpComposer.selectedServers`, so it is NOT per-pane — this spec asserts the
 * chip page-scoped, not per-pane isolation. The per-pane MCP correctness that
 * matters — the conversation-keyed send config (`getSelectedServersConfigFor`) and
 * the wrong-pane tool-approval routing (`approvalKeyOf`, the flagship ITEM-33 bug)
 * — is proven deterministically at the UNIT level (`approvalRouting.test.ts`), an
 * e2e approval needing a real MCP tool call with an approval-gated tool. No LLM.
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

  test('the MCP config surface is reachable per-pane; enabling a server for a pane applies it', async ({
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

    // Each pane's composer is independently instanced — both carry their own "+"
    // tools button, so the MCP config/menu SURFACE is reachable per-pane.
    await expect(pane0.getByTestId('chat-input-add-btn')).toBeVisible()
    await expect(pane1.getByTestId('chat-input-add-btn')).toBeVisible()

    // Open the MCP config modal from pane B's composer and enable the server.
    await pane1.click()
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })
    const toggle = byTestId(page, `mcp-config-server-switch-${serverId}`)
    if ((await toggle.getAttribute('aria-checked')) !== 'true') await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
    await byTestId(page, 'mcp-config-close-btn').click()

    // The server is now selected — its chip renders in the composer status row.
    //
    // NOTE (DRIFT-2.11): the chip DISPLAY reads the GLOBAL single-active
    // `McpComposer.selectedServers`, so it is NOT per-pane and both panes show the
    // same chip. The per-pane MCP correctness that actually matters — the
    // wrong-pane tool-approval routing and the per-conversation send config — is
    // conversation-keyed (`approvalKeyOf` / `getSelectedServersConfigFor`) and is
    // proven deterministically at the unit level by `approvalRouting.test.ts`
    // (an e2e approval needs a real MCP tool call with an approval-gated tool).
    await expect(byTestId(page, `mcp-chip-${serverId}`).first()).toBeVisible({ timeout: 10000 })
  })
})
