import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from './helpers/chat-helpers'
import { MockResourceLinkServer } from '../07-mcp/helpers/resource-link-mock-server'

/**
 * E2E (real-LLM) — the MCP tool-APPROVAL flow with a REAL model deciding to
 * call a tool that requires manual approval.
 *
 * Audit gap: mcp-tool-approval-optimistic.spec mocks the whole SSE stream; no
 * test exercised the approval gate with a real LLM choosing to call the tool.
 * Here a real Anthropic model is asked to use a mock MCP tool whose server is
 * configured manual_approve → the approval panel must surface → approving it
 * lets the tool run and its result render. Soft-skips without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — real-LLM MCP tool approval', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  let mock: MockResourceLinkServer

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('real model calls a manual-approve tool → approval panel → approve → runs', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, 'claude-haiku-4-5-20251001', 'Claude Haiku 4.5', 'anthropic')

    mock = await MockResourceLinkServer.start({ baseUrl: baseURL })
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: auth,
      data: {
        name: `mock_approval_${Date.now()}`,
        display_name: 'Mock Approval Server',
        description: 'manual-approve tool server',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        usage_mode: 'auto',
      },
    })
    const serverId = (await created.json()).id as string

    const groupsBody = await (await page.request.get(`${apiURL}/api/groups`, { headers: auth })).json()
    const groups = Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const dg = groups.find((g: { is_default?: boolean }) => g.is_default) ??
      groups.find((g: { name: string }) => g.name === 'Users')
    await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
      headers: auth,
      data: { group_ids: [dg.id] },
    })

    // MANUAL approval — the tool call must surface an approval panel.
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: auth,
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'manual_approve',
        auto_approved_tools: [],
      },
    })

    const mockedUri = '/api/files/mock/plot.png'
    await page.route(`**${mockedUri}`, async route => {
      const png = Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
        'base64',
      )
      await route.fulfill({ status: 200, contentType: 'image/png', body: png })
    })

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Use the get_file_link tool to make a PNG named "plot.png" with mime_type "image/png" available.',
      true,
    )

    // The approval panel surfaces (manual_approve) — the model chose to call the tool.
    const approveBtn = page
      .locator('[data-testid="tool-approval-approve-once"]')
      .first()
    await expect(approveBtn).toBeVisible({ timeout: 60000 })
    await approveBtn.click()

    // After approval the tool runs and its (image) result renders.
    await expect(page.locator('img').last()).toBeVisible({ timeout: 60000 })
  })
})
