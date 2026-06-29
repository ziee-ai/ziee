import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from '../chat/helpers/chat-helpers'
import { MockResourceLinkServer } from '../mcp/helpers/resource-link-mock-server'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { installMcpServerFromHub } from './helpers/hub-mcp'

/**
 * E2E (real-LLM) — a hub-installed MCP server is actually USABLE in chat.
 *
 * Audit gap (all-fd3927602356): hub-mcp.spec only proves the install flow
 * materializes a server row + badge; nothing proved an installed-from-hub MCP
 * server's tools attach to a chat and a real model can CALL one. This drives
 * the full path: install `brave-search-mcp` from the hub through the real UI
 * (which creates a user MCP server carrying the hub linkage), repoint that
 * SAME hub-created row at a loopback mock MCP server (the only external
 * boundary swapped — the manifest's real Brave URL is unreachable/keyless in
 * CI), then a real Anthropic model is asked to use the server's tool and we
 * assert the tool call fires + its result renders in chat.
 *
 * Soft-skips without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)
const HTTP_HUB_MCP_ID = 'brave-search-mcp'

test.describe('Hub MCP — installed server is usable in chat', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  let mock: MockResourceLinkServer

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('install MCP from hub → real model calls its tool → result renders', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    // Real provider + tool-capable model.
    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // Loopback mock MCP server exposing a deterministic `get_file_link` tool.
    mock = await MockResourceLinkServer.start({ baseUrl: baseURL })

    // ---- Install the MCP server FROM THE HUB via the real UI flow ----
    // "Install for me" → user-scope MCP server row created with the hub
    // linkage (hub_id forwarded through the create endpoint).
    const customName = `hubmcp-${Date.now()}`
    await navigateToHub(page, baseURL, 'mcp-servers')
    await waitForHubDataLoad(page)
    await installMcpServerFromHub(page, HTTP_HUB_MCP_ID, { name: customName })

    // Find the just-installed user server by its unique slug.
    const listResp = await page.request.get(`${apiURL}/api/mcp/servers`, { headers: auth })
    expect(listResp.ok(), `list servers: ${listResp.status()}`).toBeTruthy()
    const listBody = await listResp.json()
    const servers = Array.isArray(listBody) ? listBody : (listBody.servers ?? [])
    const installed = servers.find((s: { name: string }) => s.name === customName)
    expect(installed, `hub-installed server "${customName}" must exist in the user's MCP servers`).toBeTruthy()
    const serverId = installed.id as string

    // ---- Repoint the hub-created row at the reachable mock (external
    // boundary only) and ensure it is enabled + auto usage. ----
    const updateResp = await page.request.put(`${apiURL}/api/mcp/servers/${serverId}`, {
      headers: auth,
      data: {
        transport_type: 'http',
        url: mock.url(),
        enabled: true,
        usage_mode: 'auto',
      },
    })
    expect(updateResp.ok(), `repoint server: ${updateResp.status()}`).toBeTruthy()
    const updated = await updateResp.json()
    // The connection-health probe must keep it enabled (the mock answers
    // the MCP initialize handshake), otherwise its tools won't attach.
    expect(updated.enabled, 'mock-backed hub server must stay enabled after the health probe').toBe(true)

    // Auto-approve so the real model's tool call runs without a manual gate.
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: auth,
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'auto_approve',
        auto_approved_tools: [],
      },
    })

    // The tool returns a resource link to this URL; serve a tiny PNG for it.
    const mockedUri = '/api/files/mock/plot.png'
    await page.route(`**${mockedUri}`, async route => {
      const png = Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
        'base64',
      )
      await route.fulfill({ status: 200, contentType: 'image/png', body: png })
    })

    // ---- Real chat: the model decides to call the hub-installed server's tool ----
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Use the get_file_link tool to make a PNG named "plot.png" with mime_type "image/png" available.',
      true,
    )

    // The tool result (image) renders — proving the install→chat→tool-call path.
    await expect(page.locator('img').last()).toBeVisible({ timeout: 60000 })
    // And the mock actually received the call (not a hallucinated answer).
    expect(mock.toolCallCount()).toBeGreaterThan(0)
  })
})
