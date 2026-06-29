import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from '../mcp/helpers/navigation-helpers'
import { clickEditServerButton } from '../mcp/helpers/form-helpers'
import { MockResourceLinkServer } from '../mcp/helpers/resource-link-mock-server'

// Realtime delivery of the `mcp_tool_call` sync entity. A tool call driven on
// device A appears in the server's "Calls" tab on device B (same owner) WITHOUT
// a reload. Cross-user isolation is proven by the backend `SyncProbe` test
// (`tests/mcp/sync_emit_test.rs::tool_call_create_is_delivered_to_owner_*`).
//
// Run with --workers=1 (shared backend + DB). Never use `networkidle`.
test.describe('MCP tool-call history — realtime sync', () => {
  let mock: MockResourceLinkServer

  test.beforeEach(async ({ testInfra }) => {
    mock = await MockResourceLinkServer.start({ baseUrl: testInfra.baseURL })
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('a tool call appears live in the Calls tab on a second device', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Device A (admin) registers the mock as a server.
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)
    const createRes = await page.request.post(`${apiURL}/api/mcp/servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `tc-sync-${Date.now()}`,
        display_name: 'TC Sync Mock',
        transport_type: 'http',
        url: mock.url(),
        enabled: true,
      },
    })
    expect(createRes.ok()).toBeTruthy()
    const server = await createRes.json()

    // Device B (same owner, second context) opens the server's Calls tab and
    // is fully loaded BEFORE the mutation, so its SSE stream is live.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    await loginAsAdmin(pageB, baseURL)
    await goToMcpServersPage(pageB, baseURL)
    await waitForMcpPageLoad(pageB)
    await clickEditServerButton(pageB, 'TC Sync Mock')
    await pageB.getByTestId('mcp-drawer-tabs-tab-calls').click()
    await expect(pageB.getByTestId('mcp-tool-calls-tab')).toBeVisible()

    // Device A drives a tool call.
    const callRes = await page.request.post(
      `${apiURL}/api/mcp/servers/${server.id}/tools/get_file_link/call`,
      {
        headers: { Authorization: `Bearer ${token}` },
        data: { arguments: { name: 'live.pdf' } },
      },
    )
    expect(callRes.ok()).toBeTruthy()

    // Device B sees the new row arrive WITHOUT a reload (sync:mcp_tool_call).
    await expect(
      pageB.getByTestId(/^mcp-tool-calls-table-row-/).filter({ hasText: 'get_file_link' }),
    ).toBeVisible({ timeout: 15_000 })

    await ctxB.close()
  })
})
