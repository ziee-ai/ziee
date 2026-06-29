import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from './helpers/navigation-helpers'
import { clickEditServerButton } from './helpers/form-helpers'
import { MockResourceLinkServer } from './helpers/resource-link-mock-server'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../09-chat/helpers/chat-helpers'

/**
 * E2E (real-LLM) — a tool call made through a REAL chat surfaces in the MCP
 * tool-call history UI.
 *
 * Audit gap (all-82ecdff6b209): `mcp-tool-call-history.spec.ts` and
 * `13-sync/mcp-tool-call-sync.spec.ts` both seed the history by driving the
 * REST `/tools/{name}/call` endpoint directly — never by an actual chat where
 * a model decides to invoke the tool. This walks the full production path:
 * a real Anthropic model is asked to use a mock MCP tool (auto-approved so it
 * runs unattended), the call is recorded by the `McpSession::call_tool`
 * chokepoint, and the chat-driven invocation must then appear in that server's
 * "Calls" tab. Soft-skips without ANTHROPIC_API_KEY. Only the model's
 * decision-to-call is non-deterministic; nothing is mocked away.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('MCP tool-call history — recorded from a real chat', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  let mock: MockResourceLinkServer

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('a chat-driven tool call appears in the server Calls tab', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

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

    // Register the mock as a system MCP server (the proven path for getting a
    // tool into a real chat), assign it to the default group, and auto-approve
    // so the model's call runs unattended.
    const displayName = `TC From Chat ${Date.now()}`
    mock = await MockResourceLinkServer.start({ baseUrl: baseURL })
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: auth,
      data: {
        name: `mock_tc_chat_${Date.now()}`,
        display_name: displayName,
        description: 'chat-driven tool-call history server',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        usage_mode: 'auto',
      },
    })
    expect(created.ok()).toBeTruthy()
    const serverId = (await created.json()).id as string

    const groupsBody = await (
      await page.request.get(`${apiURL}/api/groups`, { headers: auth })
    ).json()
    const groups = Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const dg =
      groups.find((g: { is_default?: boolean }) => g.is_default) ??
      groups.find((g: { name: string }) => g.name === 'Users')
    await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
      headers: auth,
      data: { group_ids: [dg.id] },
    })

    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: auth,
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'auto_approve',
        auto_approved_tools: [],
      },
    })

    // The mock returns a resource_link to this URI; stub the bytes so the
    // result renders cleanly (this is an external asset boundary, not the
    // behavior under test).
    await page.route('**/api/files/mock/plot.png', async route => {
      const png = Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
        'base64',
      )
      await route.fulfill({ status: 200, contentType: 'image/png', body: png })
    })

    // Drive the tool call through a REAL chat — the model chooses to call it.
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Use the get_file_link tool to make a PNG named "plot.png" with mime_type "image/png" available.',
      true,
    )

    // The model actually invoked the mock's tool (the chat→tool-call path ran).
    await expect
      .poll(() => mock.toolCallCount(), { timeout: 90_000 })
      .toBeGreaterThan(0)

    // Now the chat-driven invocation must surface in the server's Calls tab —
    // the production record-and-display path the API-seeded tests never reach.
    await goToMcpAdminPage(page, baseURL)
    await waitForMcpAdminPageLoad(page)
    await clickEditServerButton(page, displayName, true)
    await page.getByTestId('mcp-drawer-tabs-tab-calls').click()

    const tab = page.getByTestId('mcp-tool-calls-tab')
    await expect(tab).toBeVisible()
    // The recorded call's tool name is dynamic data — match the table row
    // carrying it (filter, not getByText, to stay i18n/gate-safe).
    const callRow = tab
      .getByTestId(/^mcp-tool-calls-table-row-/)
      .filter({ hasText: 'get_file_link' })
    await expect(callRow).toBeVisible({ timeout: 15_000 })

    // Expanding the row shows the recorded call detail (source: chat).
    await callRow.click()
    await expect(page.getByTestId('mcp-tool-call-detail')).toBeVisible()
  })
})
