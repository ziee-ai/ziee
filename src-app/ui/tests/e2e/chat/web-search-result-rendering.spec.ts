import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../helpers/sse-mock-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
} from './helpers/chat-helpers'

/**
 * Chat-panel rendering of a `web_search` tool result. The existing
 * settings/web-search-settings.spec only covers the admin settings page;
 * this exercises how a web_search result surfaces in the chat transcript
 * (mirrors literature/screening-flow.spec's seeded-tool_result approach —
 * deterministic, no live LLM/provider). The web_search MCP emits a readable
 * text digest + typed structured_content; this asserts the digest renders.
 */
test.describe('Chat - web_search result rendering', () => {
  test('a seeded web_search tool result renders its digest in the transcript', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      undefined,
      'openai',
    )

    const toolUseId = `tu_ws_${Math.random().toString(36).slice(2, 9)}`
    const assistantMessageId = `amsg_ws_${Math.random().toString(36).slice(2, 9)}`
    const userMessageId = `umsg_ws_${Math.random().toString(36).slice(2, 9)}`
    const digest =
      'Web search: "rust async runtime" — 2 results from searxng.'

    await mockChatTokenStream(page, [
      [startedEvent({ userMessageId }), completeEvent()],
    ])

    const toolUse: MockMessageContent = {
      content_type: 'tool_use',
      content: {
        type: 'tool_use',
        id: toolUseId,
        name: 'web_search',
        server_id: 'web-search-test-server',
        input: { query: 'rust async runtime' },
      },
    }
    const toolResult: MockMessageContent = {
      content_type: 'tool_result',
      content: {
        type: 'tool_result',
        tool_use_id: toolUseId,
        name: 'web_search',
        server_id: 'web-search-test-server',
        content: digest,
        structured_content: {
          provider: 'searxng',
          query: 'rust async runtime',
          results: [
            { title: 'Tokio — async runtime', url: 'https://tokio.rs' },
            { title: 'async-std', url: 'https://async.rs' },
          ],
        },
        is_error: false,
      },
    }

    await mockGetMessages(page, [
      mockUserMessage({ id: userMessageId, text: 'search the web' }),
      {
        id: assistantMessageId,
        role: 'assistant',
        contents: [toolUse, toolResult],
      },
    ])

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = page
      .locator('textarea[placeholder*="Type your message"]')
      .first()
    await textarea.fill('search the web')
    await page.getByRole('button', { name: 'Send message' }).click()

    const assistantMsg = page
      .locator(
        `[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`,
      )
      .first()
    await assistantMsg.waitFor({ state: 'visible', timeout: 15000 })

    // The MCP tool-use renderer shows the web_search tool-use card + a
    // Completed status for the matched tool_result. Tool results are collapsed
    // by default; the digest text renders inside the expanded "Result:" panel.
    const toolCard = assistantMsg.locator(
      `[data-testid="mcp-tooluse-card-${toolUseId}"]`,
    )
    await expect(toolCard).toBeVisible({ timeout: 10000 })
    await expect(
      assistantMsg.locator(`[data-testid="mcp-tooluse-status-${toolUseId}"]`),
    ).toHaveAttribute('data-status', 'completed')

    // Expand details → the readable web_search digest renders inline.
    await assistantMsg
      .locator(`[data-testid="mcp-tooluse-details-btn-${toolUseId}"]`)
      .click()
    await expect(assistantMsg.getByText(/Web search:/)).toBeVisible()
    await expect(assistantMsg.getByText(/2 results from searxng/)).toBeVisible()
  })
})
