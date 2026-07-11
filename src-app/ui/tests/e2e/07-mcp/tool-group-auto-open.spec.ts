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
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  mcpApprovalRequiredEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageContent,
} from '../helpers/sse-mock-helpers'
import { mockToolUseContent, mockToolResultContent } from '../chat/fixtures/mock-tool-result'

/**
 * Issue 2: a ≥2-tool group must AUTO-OPEN (and stay open) while a tool is
 * pending approval, so a collapsed group can never hide the approval prompt and
 * strand the user. And it must stay user-collapsible when no such trigger
 * applies (a plain completed run is collapsed by default, still expandable).
 */
test.describe('MCP tool group — auto-open', () => {
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

  test('a pending approval inside a 2-tool group forces the group open (approval actionable)', async ({
    page,
    testInfra,
  }) => {
    const userMsgId = 'umsg_ao_appr'
    const assistantMsgId = 'amsg_ao_appr'
    const useA = 'tu_ao_A'
    const useB = 'tu_ao_B'
    const serverId = 'srv-ao'

    // Tool A completes; tool B requires approval. Two tool_use blocks → group.
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: userMsgId }),
        mcpToolStartEvent({ toolUseId: useA, toolName: 'search', server: serverId }),
        mcpToolCompleteEvent({ toolUseId: useA, isError: false, result: { ok: true } }),
        mcpApprovalRequiredEvent({
          toolUseId: useB,
          toolName: 'dangerous_op',
          server: serverId,
          input: { target: 'production' },
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
    ])

    // Persisted reload keeps both tool_use blocks alive; the store still holds
    // A=completed, B=pending_approval, so the group renders with B's approval.
    const contents: MockMessageContent[] = [
      mockToolUseContent({ toolUseId: useA, toolName: 'search', serverId }),
      mockToolResultContent({ toolUseId: useA, toolName: 'search', resourceLinks: [] }),
      mockToolUseContent({ toolUseId: useB, toolName: 'dangerous_op', serverId, input: { target: 'production' } }),
    ]
    await mockGetMessages(page, [
      mockUserMessage({ id: userMsgId, text: 'run two ops' }),
      { id: assistantMsgId, role: 'assistant', contents },
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await sendChatMessage(page, 'run two ops')

    // The group is present AND the approval panel for the pending tool is visible
    // WITHOUT clicking the group's expand toggle — the anti-stuck-user guarantee.
    const group = page.locator('[data-testid="mcp-toolgroup-card"]').first()
    await expect(group).toBeVisible({ timeout: 15000 })
    const approval = group.locator(`[data-testid="tool-approval-${useB}"]`)
    await expect(approval).toBeVisible({ timeout: 15000 })
    await expect(
      group.locator('[data-testid="tool-approval-approve-once"]'),
    ).toBeVisible()
  })

  test('a completed run with no artifact is collapsed by default and still expandable', async ({
    page,
    testInfra,
  }) => {
    const userMsgId = 'umsg_ao_plain'
    const assistantMsgId = 'amsg_ao_plain'
    const useA = 'tu_aop_A'
    const useB = 'tu_aop_B'
    const serverId = 'srv-aop'

    // Deterministic: a minimal stream (no tool events) + a persisted 2-tool run.
    // The McpComposer store holds NO toolCalls, so hasRunning/hasPendingApproval
    // are false and there is no artifact → no auto-open trigger → the group
    // renders collapsed on the reload, with no streaming 'started' latch to leak.
    await mockChatTokenStream(page, [
      [startedEvent({ userMessageId: userMsgId }), completeEvent()],
    ])
    const contents: MockMessageContent[] = [
      mockToolUseContent({ toolUseId: useA, toolName: 'search', serverId }),
      mockToolResultContent({ toolUseId: useA, toolName: 'search', resourceLinks: [] }),
      mockToolUseContent({ toolUseId: useB, toolName: 'lookup', serverId }),
      mockToolResultContent({ toolUseId: useB, toolName: 'lookup', resourceLinks: [] }),
    ]
    await mockGetMessages(page, [
      mockUserMessage({ id: userMsgId, text: 'look two things up' }),
      { id: assistantMsgId, role: 'assistant', contents },
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await sendChatMessage(page, 'look two things up')

    const group = page
      .locator(`[data-testid="chat-message"][data-message-id="${assistantMsgId}"]`)
      .locator('[data-testid="mcp-toolgroup-card"]')
      .first()
    await expect(group).toBeVisible({ timeout: 15000 })

    // Collapsed by default: the per-tool cards are NOT rendered until expanded.
    const innerCards = group.locator(
      '[data-testid^="mcp-toolcall-card-"], [data-testid^="mcp-tooluse-card-"]',
    )
    await expect(innerCards).toHaveCount(0)

    // Still user-expandable via the toggle.
    await group.locator('[data-testid="mcp-toolgroup-details-btn"]').click()
    await expect(innerCards.first()).toBeVisible({ timeout: 10000 })
  })
})
