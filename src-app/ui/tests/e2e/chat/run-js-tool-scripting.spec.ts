import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from '../mcp/helpers/navigation-helpers'
import { clickEditServerButton } from '../mcp/helpers/form-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  mockAssistantToolUseMessage,
} from '../helpers/sse-mock-helpers'

/**
 * run_js primary flow + history source tag (TEST-35 / TEST-32).
 *
 * PTC economics: a `run_js` script loops read-only sub-tools IN-PROCESS and only
 * its FINAL value returns to context — the chat therefore renders ONE run_js tool
 * card carrying the summary, not N intermediate tool cards. The sub-tool calls it
 * makes are recorded with `source='script'` and surface in the server Calls tab.
 *
 * The mocked-SSE stream is the LLM boundary only; the tool-card renderer, the
 * McpToolCallsTab, and the source-tag styling are the REAL components. (The
 * backend recording of `source='script'` is proven end-to-end by the integration
 * tier — tests/js_tool/mod.rs TEST-9/15; here we prove the frontend rendering.)
 */
test.describe('run_js tool scripting', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  // TEST-35 — the model emits a single run_js call whose script loops a tool;
  // the chat renders ONE run_js card with the final summary (not N cards).
  test('renders ONE run_js card carrying the final summary', async ({ page, testInfra }) => {
    const toolUseId = 'tu-runjs-1'
    const script =
      'let n=0; for (const id of [1,2,3]) { const r = await ziee.tools.get_tool_result({tool_use_id: String(id)}); n++; } return `processed ${n} items`'

    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg-rjs-1' }),
        mcpToolStartEvent({
          toolUseId,
          toolName: 'run_js',
          server: 'run_js',
          input: { script },
        }),
        mcpToolCompleteEvent({
          toolUseId,
          result: {
            result: 'processed 3 items across the batch',
            console: [],
            tool_calls: [
              { name: 'get_tool_result', server: 'tool_result' },
              { name: 'get_tool_result', server: 'tool_result' },
              { name: 'get_tool_result', server: 'tool_result' },
            ],
          },
        }),
        completeEvent({ finishReason: 'end_turn' }),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg-rjs-1', text: 'summarize the batch with a script' }),
      mockAssistantToolUseMessage({
        id: 'amsg-rjs-1',
        toolUseId,
        toolName: 'run_js',
        serverId: 'run_js',
        input: { script },
      }),
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await sendChatMessage(page, 'summarize the batch with a script')

    // Exactly ONE tool card — the run_js call — and it names run_js.
    const card = page.locator(`[data-testid="mcp-toolcall-card-${toolUseId}"]`).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await expect(page.locator('[data-testid^="mcp-toolcall-card-"]')).toHaveCount(1)
    // The tool-name span is exactly "run_js" (the server label is "(run_js)").
    await expect(card.getByText('run_js', { exact: true })).toBeVisible()

    // It reached the completed state.
    await expect(
      page.locator(`[data-testid="mcp-toolcall-status-${toolUseId}"]`),
    ).toHaveAttribute('data-status', 'completed')

    // The final summary is the card's result (expand to reveal the JSON body).
    await page.locator(`[data-testid="mcp-toolcall-details-btn-${toolUseId}"]`).click()
    await expect(card.getByText('processed 3 items across the batch', { exact: false })).toBeVisible()
  })

  // TEST-32 — the sub-tool calls a run_js script makes are recorded with
  // source='script'; the server Calls tab renders that source tag (its own
  // tone/label, not the fallback default). The tool-calls REST endpoint is the
  // mocked data boundary; the McpToolCallsTab rendering is real.
  test('Calls tab renders the script source tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getCurrentUserToken(page)

    // A user MCP server whose Calls tab we open (any server hosts the tab).
    const createRes = await page.request.post(`${apiURL}/api/mcp/servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `rjs-src-${Date.now()}`,
        display_name: 'RunJS Source Mock',
        transport_type: 'http',
        url: 'http://127.0.0.1:1/mcp',
        enabled: false,
      },
    })
    expect(createRes.ok()).toBeTruthy()
    const server = await createRes.json()

    const rowId = 'tc-script-row-1'
    const now = new Date().toISOString()
    await page.route('**/api/mcp/tool-calls?**', async route => {
      if (route.request().method() !== 'GET') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          calls: [
            {
              id: rowId,
              user_id: 'u1',
              server_id: server.id,
              server_name: 'web_search',
              tool_name: 'web_search',
              source: 'script',
              status: 'completed',
              is_error: false,
              is_built_in: false,
              content_kinds: ['text'],
              result_bytes: 128,
              arguments_json: { query: 'x' },
              created_at: now,
              started_at: now,
              updated_at: now,
              finished_at: now,
              duration_ms: 42,
            },
          ],
          page: 1,
          per_page: 20,
          total: 1,
          total_pages: 1,
        }),
      })
    })

    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)
    await clickEditServerButton(page, 'RunJS Source Mock')
    await page.getByTestId('mcp-drawer-tabs-tab-calls').click()

    await expect(page.getByTestId('mcp-tool-calls-tab')).toBeVisible()
    const sourceTag = page.getByTestId(`mcp-tool-call-source-${rowId}`)
    await expect(sourceTag).toBeVisible({ timeout: 15000 })
    await expect(sourceTag).toHaveText('script')
    // Rendered with the `script` source's OWN tone (SOURCE_TONE.script = 'info' →
    // `text-info`), not the fallback `default` tone (`text-foreground/80`).
    await expect(sourceTag).toHaveClass(/text-info/)
  })
})

async function sendChatMessage(page: import('@playwright/test').Page, text: string) {
  const textarea = page.locator('[data-testid="chat-message-textarea"]').first()
  await textarea.fill(text)
  await page.locator('[data-testid="chat-input-send-btn"]').click()
}
