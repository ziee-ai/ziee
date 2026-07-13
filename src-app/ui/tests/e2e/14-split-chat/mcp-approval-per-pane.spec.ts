import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  mcpApprovalRequiredEvent,
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  mockAssistantToolUseMessage,
} from '../helpers/sse-mock-helpers'

/**
 * Split-chat E2E — MCP tool-APPROVAL routes to the pane that owns it (TEST-53b,
 * audit #5). The flagship ITEM-33 bug: approving a tool in pane B called the
 * FOCUSED-pane bridge's sendMessage, so the resume posted to pane A. This drives a
 * real approval round-trip in pane B while pane A is FOCUSED and asserts the
 * approval RESUME send targets pane B's conversation (captured send URL), not the
 * focused pane A's — and that pane A receives nothing. The tool call is mocked at
 * the SSE boundary; the routing (the crux) is real. Complements the unit
 * `approvalRouting.test.ts`.
 */
test.describe('Split chat — per-pane MCP tool approval routing', () => {
  test('approving in pane B (pane A focused) resumes in pane B, not the focused pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('Approve Alpha')
    const convB = await mkConv('Approve Bravo')

    const toolUseId = 'tu_split_approve'
    const serverId = 'mock-server'
    const mock = await mockChatTokenStream(page, [
      // Send #1 (pane B): surface an approval-required tool call.
      [
        startedEvent({ userMessageId: 'umsg_b1' }),
        mcpApprovalRequiredEvent({ toolUseId, toolName: 'dangerous_op', server: serverId, input: { target: 'B' } }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
      // Send #2 (the RESUME after approve): complete the tool.
      [
        startedEvent({ userMessageId: 'umsg_b_resume' }),
        mcpToolStartEvent({ toolUseId, toolName: 'dangerous_op', server: serverId }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: { ok: true } }),
        textDeltaEvent({ delta: 'Tool done in B.' }),
        completeEvent(),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_b1', text: 'run the dangerous op' }),
      mockAssistantToolUseMessage({ id: 'amsg_b1', toolUseId, toolName: 'dangerous_op', serverId, input: { target: 'B' } }),
    ])
    // `mockGetMessages` is a catch-all; scope convA to EMPTY (added after → matched
    // first) so pane A doesn't render the tool card from the shared mock set.
    await page.route(new RegExp(`/api/conversations/${convA}/messages(\\?|$)`), async (route, req) => {
      if (req.method() !== 'GET') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ messages: [], has_more_before: false, has_more_after: false }),
      })
    })

    // [A | B] split.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })

    // Send in pane B → the approval card appears in pane B.
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputB.click()
    await inputB.fill('run the dangerous op')
    await pane1.getByRole('button', { name: 'Send message' }).click()
    await expect(pane1.getByTestId('tool-approval-approve-once')).toBeVisible({ timeout: 30000 })
    // Pane A never shows an approval card.
    await expect(pane0.getByTestId('tool-approval-approve-once')).toHaveCount(0)

    // Focus pane A, THEN approve in pane B (the bug's trigger: focused ≠ owner).
    await pane0.click()
    await expect(pane0).toHaveClass(/opacity-100/)
    await pane1.getByTestId('tool-approval-approve-once').click()

    // CRUX: the resume send routed to pane B's conversation, not the focused A's.
    await expect
      .poll(() => mock.capturedSends().length, { timeout: 15000 })
      .toBeGreaterThanOrEqual(2)
    const sends = mock.capturedSends()
    const resume = sends[sends.length - 1]
    expect(resume.url).toContain(convB)
    expect(resume.url).not.toContain(convA)
    // Every send in this test targeted B (pane A never posted).
    expect(sends.every(s => s.url.includes(convB))).toBeTruthy()
  })
})
