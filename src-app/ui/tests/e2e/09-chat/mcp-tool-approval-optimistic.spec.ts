import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatStream,
  startedEvent,
  mcpApprovalRequiredEvent,
  mcpToolStartEvent,
  mcpToolCompleteEvent,
  textDeltaEvent,
  completeEvent,
  serializeSseScript,
  mockGetMessages,
  mockUserMessage,
  mockAssistantToolUseMessage,
} from '../helpers/sse-mock-helpers'

/**
 * ToolCallPendingApprovalContent's optimistic-update refactor in
 * feat/mcp-rewrite-v2:
 *
 * - On approve/deny click, the store status flips immediately
 *   (pending_approval → started for approve, → error for deny) so the
 *   approval panel disappears BEFORE the backend round-trip completes.
 * - If the resume sendMessage() call fails, the status is reverted back to
 *   pending_approval so the panel reappears.
 *
 * Tests use page.route to mock /messages/stream and capture optimistic
 * state in the brief window before the mock returns.
 */

test.describe('MCP Tool Approval — optimistic UX', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('approve once: panel disappears optimistically, then tool completes', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_approve_1'
    const assistantMessageId = 'amsg_approve_1'
    const serverId = 'mock-server'

    // Two-call sequence: first call surfaces approval; second call resumes.
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_approve_1' }),
        mcpApprovalRequiredEvent({
          toolUseId,
          toolName: 'dangerous_op',
          server: serverId,
          input: { target: 'production' },
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
      [
        startedEvent({ userMessageId: 'umsg_approve_resume' }),
        mcpToolStartEvent({ toolUseId, toolName: 'dangerous_op', server: serverId }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: { ok: true } }),
        textDeltaEvent({ delta: 'Done.' }),
        completeEvent(),
      ],
    ])

    // Mock the loadMessages reload after each SSE complete: keeps the tool_use
    // content block alive in the messages map so McpToolUseRenderer can mount.
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_approve_1', text: 'Please run the dangerous op' }),
      mockAssistantToolUseMessage({
        id: assistantMessageId,
        toolUseId,
        toolName: 'dangerous_op',
        serverId,
        input: { target: 'production' },
      }),
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await sendChatMessage(page, 'Please run the dangerous op')

    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`).first()
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.locator('[data-testid="tool-approval-approve-once"]').first().click()

    // Optimistic: panel must disappear immediately, BEFORE the second
    // /messages/stream call resolves. The mock returns synchronously so this
    // is best-effort; correctness is asserted by the eventual completed state.
    await expect(approval).not.toBeVisible({ timeout: 5000 })

    // Final state: tool shows as Completed
    await expect(page.locator('text=Completed').first()).toBeVisible({ timeout: 5000 })
  })

  test('approve once: panel reappears when backend resume call fails', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_approve_fail'
    const assistantMessageId = 'amsg_approve_fail'
    const serverId = 'mock-server'

    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_fail_1', text: 'Please run the risky op' }),
      mockAssistantToolUseMessage({
        id: assistantMessageId,
        toolUseId,
        toolName: 'risky_op',
        serverId,
      }),
    ])

    await goToNewChatPage(page, testInfra.baseURL)

    // First call: approval required. Second call: HTTP 500 (simulates resume failure).
    let callIndex = 0
    await page.route(/\/api\/conversations\/[^/]+\/messages\/stream(\?|$)/, async (route, request) => {
      // Only intercept POST — let GETs fall through to mockGetMessages (above).
      if (request.method() !== 'POST') {
        return route.fallback()
      }
      callIndex++
      if (callIndex === 1) {
        const body = serializeSseScript([
          startedEvent({ userMessageId: 'umsg_fail_1' }),
          mcpApprovalRequiredEvent({
            toolUseId,
            toolName: 'risky_op',
            server: 'mock-server',
            input: {},
          }),
          completeEvent({ finishReason: 'tool_use' }),
        ])
        await route.fulfill({ status: 200, contentType: 'text/event-stream', body })
      } else {
        // Resume call fails
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'simulated resume failure' }),
        })
      }
    })

    await sendChatMessage(page, 'Please run the risky op')
    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`).first()
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.locator('[data-testid="tool-approval-approve-once"]').first().click()

    // After the resume call fails, the optimistic update is reverted →
    // approval panel reappears.
    await expect(approval).toBeVisible({ timeout: 10000 })
  })

  test('deny: panel switches to denied state immediately (optimistic error status)', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_deny_1'
    const assistantMessageId = 'amsg_deny_1'
    const serverId = 'mock-server'

    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_deny_1', text: 'Try the unwanted op' }),
      mockAssistantToolUseMessage({
        id: assistantMessageId,
        toolUseId,
        toolName: 'unwanted_op',
        serverId,
      }),
    ])

    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_deny_1' }),
        mcpApprovalRequiredEvent({
          toolUseId,
          toolName: 'unwanted_op',
          server: serverId,
          input: {},
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
      // Resume call after deny — minimal happy path
      [
        startedEvent({ userMessageId: 'umsg_deny_resume' }),
        textDeltaEvent({ delta: 'I respect your decision.' }),
        completeEvent(),
      ],
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await sendChatMessage(page, 'Try the unwanted op')
    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`).first()
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.locator('[data-testid="tool-approval-deny"]').first().click()

    // Approval panel disappears (status flipped to 'error')
    await expect(approval).not.toBeVisible({ timeout: 5000 })

    // The denied tool shows the Failed card text
    await expect(page.locator('text=Failed').first()).toBeVisible({ timeout: 5000 })
  })

  test('approve-for-conversation triggers a POST to /mcp-settings with auto_approved_tools', async ({
    page,
    testInfra,
  }) => {
    const toolUseId = 'tu_approve_conv'
    const serverId = 'srv_approve_conv'
    const assistantMessageId = 'amsg_approve_conv'

    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_conv_1', text: 'Approve me for the whole conversation' }),
      mockAssistantToolUseMessage({
        id: assistantMessageId,
        toolUseId,
        toolName: 'auto_me',
        serverId,
      }),
    ])

    await goToNewChatPage(page, testInfra.baseURL)

    // Capture all /mcp-settings PUTs
    const settingsBodies: unknown[] = []
    await page.route(/\/api\/conversations\/[^/]+\/mcp-settings/, async (route, req) => {
      const url = req.url()
      const convMatch = url.match(/\/conversations\/([^/]+)\/mcp-settings/)
      const conversationId = convMatch?.[1] ?? '00000000-0000-0000-0000-000000000000'
      let parsedBody: Record<string, unknown> = {}
      if (req.method() === 'PUT') {
        try {
          parsedBody = JSON.parse(req.postData() || '{}')
          settingsBodies.push(parsedBody)
        } catch {
          /* ignore */
        }
      }
      // Return the FULL ConversationMcpSettingsResponse shape — the
      // frontend updates its store from the PUT response, so a
      // `{ success: true }` stub leaves every field undefined and
      // breaks the optimistic-update flow we're testing.
      const now = new Date().toISOString()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: '00000000-0000-0000-0000-000000000aa1',
          conversation_id: conversationId,
          user_id: '00000000-0000-0000-0000-000000000aa2',
          approval_mode: (parsedBody.approval_mode as string) ?? 'manual_approve',
          auto_approved_tools: (parsedBody.auto_approved_tools as unknown[]) ?? [],
          disabled_servers: (parsedBody.disabled_servers as unknown[]) ?? [],
          loop_settings: (parsedBody.loop_settings as Record<string, unknown>) ?? {
            stop_when_no_tool_calling: true,
            max_iteration: 10,
            stop_when_tools_called: [],
            force_final_answer: false,
            per_tool_max_iteration: [],
          },
          created_at: now,
          updated_at: now,
        }),
      })
    })

    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_conv_1' }),
        mcpApprovalRequiredEvent({
          toolUseId,
          toolName: 'auto_me',
          server: 'mock-server',
          serverId,
          input: {},
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
      [
        startedEvent({ userMessageId: 'umsg_conv_resume' }),
        mcpToolStartEvent({ toolUseId, toolName: 'auto_me', server: 'mock-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: {} }),
        completeEvent(),
      ],
    ])

    await sendChatMessage(page, 'Approve me for the whole conversation')
    await expect(page.locator(`[data-testid="tool-approval-${toolUseId}"]`).first()).toBeVisible({
      timeout: 10000,
    })

    await page.locator('[data-testid="tool-approval-approve-conv"]').first().click()

    // Wait for the PUT to fire — it may take up to a couple seconds for the
    // sendMessage() call chain to issue the settings save.
    await page.waitForTimeout(2000)

    const matchingBody = settingsBodies.find(
      b =>
        typeof (b as Record<string, unknown>).auto_approved_tools !== 'undefined',
    ) as { auto_approved_tools?: unknown[] } | undefined

    expect(matchingBody, 'expected at least one PUT with auto_approved_tools').toBeDefined()
    expect(Array.isArray(matchingBody!.auto_approved_tools)).toBe(true)
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

async function sendChatMessage(page: import('@playwright/test').Page, text: string) {
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill(text)
  const sendButton = page.getByRole('button', { name: 'Send message' })
  await sendButton.click()
}

