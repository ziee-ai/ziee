import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from './helpers/chat-helpers'
import { MockSamplingServer } from '../07-mcp/helpers/sampling-mock-server'

/**
 * Chat-level MCP sampling — full end-to-end with a Node mock MCP server
 * (no Rust dependency) and a real Anthropic LLM. Mirrors the backend
 * `tests/chat/mcp_sampling_test.rs` integration test, but exercises the
 * full UI: select model → send chat → tool fires → 2 sampling roundtrips →
 * tool result → final assistant text.
 *
 * Tests skip cleanly when ANTHROPIC_API_KEY is not set.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — MCP sampling (real LLM + mock server)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping chat-level sampling tests')

  // These tests do real LLM API calls; mark as slow so Playwright extends timeouts.
  test.slow()

  let mock: MockSamplingServer

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Real Anthropic provider + Claude model
    const token = await getAdminToken(page)
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

    // Mock MCP server
    mock = await MockSamplingServer.start()

    // Register the mock as a system MCP server with supports_sampling
    const created = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `mock_sampling_${Date.now()}`,
        display_name: 'Mock Sampling',
        description: 'Node mock for chat-level sampling e2e',
        enabled: true,
        transport_type: 'http',
        url: mock.url(),
        timeout_seconds: 120,
        supports_sampling: true,
        usage_mode: 'auto',
      },
    })
    const serverBody = await created.json()
    const serverId: string = serverBody.id

    // Assign to default group so admin can access it
    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> =
      Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const defaultGroup = groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (defaultGroup) {
      await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { group_ids: [defaultGroup.id] },
      })
    }

    // Select the mock server in user defaults + set auto-approve so the tool
    // call doesn't block on user approval
    await page.request.put(`${apiURL}/api/mcp/defaults`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        selected_servers: [{ server_id: serverId, tools: [] }],
        disabled_servers: [],
        approval_mode: 'auto_approve',
        auto_approved_tools: [],
      },
    })
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  // Chat.store now creates a placeholder streaming message on the
  // first delta of any type (text/thinking/tool_use), so the assistant
  // bubble renders immediately even on tool-first flows — fixing the
  // TODO_E2E.md item-5 wait timeout that previously skipped this test.
  test('research tool triggers two sampling roundtrips and returns a final answer', async ({
    page,
    testInfra,
  }) => {
    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      "Use the research tool to find the capital of France. Query: 'capital of France'.",
      true,
    )

    // Wait an extra moment for the second sampling roundtrip + final assistant text to settle
    await page.waitForTimeout(3000)

    // The mock server must have processed both sampling responses
    expect(mock.samplingCallCount()).toBe(2)

    // The chat should contain an assistant message mentioning Paris
    const assistantText = await page
      .locator('[data-role="assistant"]')
      .last()
      .textContent()
    expect(assistantText?.toLowerCase() ?? '').toContain('paris')
  })

  test('Sampling badge is visible on the mock server card on the user MCP page', async ({
    page,
    testInfra,
  }) => {
    await page.goto(`${testInfra.baseURL}/settings/mcp-servers`)
    await page.waitForLoadState('load')
    // Find Mock Sampling card; verify it carries the Sampling badge
    const card = page.getByTestId(/^mcp-server-card-/).filter({ hasText: 'Mock Sampling' }).first()
    await expect(card).toBeVisible({ timeout: 10000 })
    await expect(card.locator('[data-testid="mcp-sampling-badge"]')).toBeVisible()
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}
