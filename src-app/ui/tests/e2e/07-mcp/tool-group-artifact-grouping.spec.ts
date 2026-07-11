import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from '../chat/helpers/chat-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../helpers/sse-mock-helpers'
import {
  mockToolUseContent,
  mockToolResultContent,
  mockBackendFile,
} from '../chat/fixtures/mock-tool-result'

/**
 * Issue 1 + 3: an MCP-server artifact `tool_result` that is persisted NON-adjacent
 * to its producing `tool_use` (a `text` block sits between the tool run and the
 * artifact result — the exact shape that used to make the artifact "escape" the
 * "N tools called" box) must be:
 *   (a) WRAPPED inside the `mcp-toolgroup-card` (grouping is by tool_use_id, not
 *       positional adjacency — normalizeToolResultOrder), and
 *   (b) VISIBLE without a click because a run that produced an artifact AUTO-OPENS.
 */
test.describe('MCP tool group — artifact grouping + auto-open', () => {
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

  test('a non-adjacent artifact tool_result is wrapped in the group box and the group auto-opens', async ({
    page,
    testInfra,
  }) => {
    const userMsgId = 'umsg_tag'
    const assistantMsgId = 'amsg_tag'
    const useA = 'tu_tag_A'
    const useB = 'tu_tag_B'
    const artifactFileId = 'tag-artifact-1'
    const artifactUri = `/api/files/${artifactFileId}`

    // Backend-owned CSV artifact so the inline preview resolves + renders.
    await mockBackendFile(page, {
      fileId: artifactFileId,
      filename: 'data.csv',
      mimeType: 'text/csv',
      textContent: 'a,b\n1,2\n',
    })

    // Minimal stream — the persisted /messages reload is what renders the final
    // block layout we assert on.
    await mockChatTokenStream(page, [
      [startedEvent({ userMessageId: userMsgId }), completeEvent()],
    ])

    // Persisted order: [use_A, result_A, use_B, TEXT, result_B(artifact)].
    // result_B sits AFTER a non-tool block → pre-fix it would render unwrapped,
    // standalone next to the 2-tool group.
    const contents: MockMessageContent[] = [
      mockToolUseContent({ toolUseId: useA, toolName: 'search', serverId: 'srv-tag' }),
      mockToolResultContent({ toolUseId: useA, toolName: 'search', resourceLinks: [] }),
      mockToolUseContent({ toolUseId: useB, toolName: 'make_chart', serverId: 'srv-tag' }),
      { content_type: 'text', content: { type: 'text', text: 'Here is your chart.' } },
      mockToolResultContent({
        toolUseId: useB,
        toolName: 'make_chart',
        resourceLinks: [
          { uri: artifactUri, name: 'data.csv', mime_type: 'text/csv', file_id: artifactFileId },
        ],
      }),
    ]

    await mockGetMessages(page, [
      mockUserMessage({ id: userMsgId, text: 'do two things' }),
      { id: assistantMsgId, role: 'assistant', contents },
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await sendChatMessage(page, 'do two things')

    const bubble = page.locator(
      `[data-testid="chat-message"][data-message-id="${assistantMsgId}"]`,
    )
    await expect(bubble).toBeVisible({ timeout: 15000 })

    // Exactly one group card (2 tools folded together).
    const group = bubble.locator('[data-testid="mcp-toolgroup-card"]')
    await expect(group).toHaveCount(1)

    // The artifact renders INSIDE the group card WITHOUT clicking the toggle:
    //  - inside the card  → it is wrapped (grouping by tool_use_id worked)
    //  - visible w/o click → the group auto-opened for the artifact
    const artifactInGroup = group.locator('[data-testid="tool-result-files"]')
    await expect(artifactInGroup).toBeVisible({ timeout: 10000 })

    // And it is the ONLY tool-result-files group (not also rendered standalone).
    await expect(bubble.locator('[data-testid="tool-result-files"]')).toHaveCount(1)
  })
})
