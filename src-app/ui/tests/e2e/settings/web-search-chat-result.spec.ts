import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
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
} from '../chat/helpers/chat-helpers'
import { byTestId } from '../testid.ts'

// Deterministic in-chat rendering of a `web_search` tool result. The
// web-search-settings spec only covers the admin settings page; this exercises
// the OTHER half — the chat surface where the model's web_search call + its
// result render inline (the generic MCP tool-use renderer: tool name, server,
// Completed status, and the digest behind "Show details"). No live LLM: the
// persisted tool_use + tool_result blocks (carrying structured_content) are
// delivered via the post-`complete` /messages reload, the real production path.

test.describe('Web search in-chat tool result', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('web_search tool call + result render inline in the conversation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    const toolUseId = `tu_ws_${Math.random().toString(36).slice(2, 9)}`
    const assistantMessageId = `amsg_ws_${Math.random().toString(36).slice(2, 9)}`
    const userMessageId = `umsg_ws_${Math.random().toString(36).slice(2, 9)}`
    const digest =
      'Web search: "ziee release notes" — SENTINEL_WS_RESULT: ziee 1.2 ships realtime sync.'

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
        input: { query: 'ziee release notes', max_results: 3 },
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
          query: 'ziee release notes',
          results: [
            {
              title: 'Ziee 1.2 release notes',
              url: 'https://example.com/ziee-1-2',
              snippet: 'Realtime sync and more.',
            },
          ],
        },
        is_error: false,
      },
    }

    await mockGetMessages(page, [
      mockUserMessage({ id: userMessageId, text: 'search the web for ziee release notes' }),
      {
        id: assistantMessageId,
        role: 'assistant',
        contents: [toolUse, toolResult],
      },
    ])

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await byTestId(page, 'chat-message-textarea')
      .first()
      .fill('search the web for ziee release notes')
    await byTestId(page, 'chat-input-send-btn').click()

    const assistantMsg = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
      .first()
    await assistantMsg.waitFor({ state: 'visible', timeout: 15000 })

    // The MCP tool-use renderer shows the web_search tool-use card (the tool
    // name `web_search` is dynamic data the mock provided) + a Completed status
    // for the matched tool_result.
    const toolCard = byTestId(assistantMsg, `mcp-tooluse-card-${toolUseId}`)
    await expect(toolCard).toBeVisible()
    await expect(toolCard.getByText('web_search').first()).toBeVisible()
    await expect(
      byTestId(assistantMsg, `mcp-tooluse-status-${toolUseId}`),
    ).toHaveAttribute('data-status', 'completed')

    // Expand details → the digest text (the readable web_search result channel)
    // renders inline (SENTINEL_WS_RESULT is dynamic data set on the tool_result).
    await byTestId(assistantMsg, `mcp-tooluse-details-btn-${toolUseId}`).click()
    await expect(assistantMsg.getByText(/SENTINEL_WS_RESULT/)).toBeVisible()
  })
})
