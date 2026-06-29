import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from './helpers/navigation-helpers'
import { clickEditServerButton } from './helpers/form-helpers'
import { MockResourceLinkServer } from './helpers/resource-link-mock-server'

// Deterministic, no LLM: register a mock MCP server, drive a tool call through
// the REST endpoint (the same `McpSession::call_tool` chokepoint the chat path
// uses), then assert the recorded row shows in the server's "Calls" tab.
//
// Never `waitForLoadState('networkidle')` — the realtime-sync SSE keeps the
// network busy forever; wait on stable selectors instead.
test.describe('MCP tool-call history', () => {
  let mock: MockResourceLinkServer

  test.beforeEach(async ({ testInfra }) => {
    mock = await MockResourceLinkServer.start({ baseUrl: testInfra.baseURL })
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('records a tool call and lists it in the server Calls tab', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    // Register the mock as a user-owned MCP server.
    const createRes = await page.request.post(`${apiURL}/api/mcp/servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `tc-e2e-${Date.now()}`,
        display_name: 'TC E2E Mock',
        transport_type: 'http',
        url: mock.url(),
        enabled: true,
      },
    })
    expect(createRes.ok()).toBeTruthy()
    const server = await createRes.json()

    // Drive a tool call (the mock exposes a `get_file_link` tool).
    const callRes = await page.request.post(
      `${apiURL}/api/mcp/servers/${server.id}/tools/get_file_link/call`,
      {
        headers: { Authorization: `Bearer ${token}` },
        data: { arguments: { name: 'report.pdf', mime_type: 'application/pdf' } },
      },
    )
    expect(callRes.ok()).toBeTruthy()

    // Open the server's edit drawer → Calls tab.
    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)
    await clickEditServerButton(page, 'TC E2E Mock')
    await page.getByTestId('mcp-drawer-tabs-tab-calls').click()

    // The recorded call appears (insert is fire-and-forget; allow a beat).
    const tab = page.getByTestId('mcp-tool-calls-tab')
    await expect(tab).toBeVisible()
    // The recorded call's tool name is dynamic data — match the table row
    // carrying it (filter, not getByText, to stay i18n/gate-safe).
    const callRow = tab
      .getByTestId(/^mcp-tool-calls-table-row-/)
      .filter({ hasText: 'get_file_link' })
    await expect(callRow).toBeVisible({ timeout: 15_000 })

    // Expanding the row shows the rest/completed metadata.
    await callRow.click()
    await expect(page.getByTestId('mcp-tool-call-detail')).toBeVisible()
  })
})
