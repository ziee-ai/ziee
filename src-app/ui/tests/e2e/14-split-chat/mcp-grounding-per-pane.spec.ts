import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane MCP grounding chips (TEST-71, ITEM-47). Selecting an
 * MCP server for one split pane's conversation shows its chip in THAT pane only;
 * the other pane's status row reflects its OWN conversation (the chip display now
 * resolves the per-conversation config, not the single global-active selection).
 * No LLM.
 */
test.describe('Split chat — per-pane MCP grounding chips', () => {
  test.describe.configure({ retries: 1 })

  test('an MCP server selected for pane B shows its chip in pane B only', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const stamp = Date.now()
    const srvRes = await page.request.post(`${apiURL}/api/mcp/servers`, {
      headers: { 'Content-Type': 'application/json', ...auth },
      data: {
        name: `e2e-ground-mcp-${stamp}`,
        display_name: `Ground MCP ${stamp}`,
        transport_type: 'http',
        url: 'https://e2e-ground-mcp.example.invalid/mcp',
        enabled: true,
      },
    })
    expect(srvRes.ok()).toBeTruthy()
    const serverId = (await srvRes.json()).id as string

    const mkConv = async (title: string) =>
      (
        await (
          await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title } })
        ).json()
      ).id as string
    const convA = await mkConv('MCP Ground A')
    const convB = await mkConv('MCP Ground B')

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(
      pane1.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })

    // Enable the server for pane B's conversation via its composer's MCP config.
    await pane1.click()
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'chat-mcp-menu-item').first().click()
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({ timeout: 10000 })
    const toggle = byTestId(page, `mcp-config-server-switch-${serverId}`)
    if ((await toggle.getAttribute('aria-checked')) !== 'true') await toggle.click()
    await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
    await byTestId(page, 'mcp-config-close-btn').click()

    // The chip renders in pane B's status row ONLY — pane A shows its own (none).
    await expect(pane1.getByTestId(`mcp-chip-${serverId}`)).toBeVisible({ timeout: 10000 })
    await expect(pane0.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0)

    // Focusing pane A does not surface B's server (per-conversation, not focus).
    await pane0.click({ position: { x: 200, y: 80 } })
    await expect(pane0.getByTestId(`mcp-chip-${serverId}`)).toHaveCount(0)
    await expect(pane1.getByTestId(`mcp-chip-${serverId}`)).toBeVisible()
  })
})
