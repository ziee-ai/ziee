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

// SKIPPED: same architectural mismatch as the elicitation specs —
// page.route on /messages/stream alone is insufficient because the chat
// store reloads messages from the real backend after the stream ends,
// wiping the optimistic tool-call state.
//
// The optimistic-update logic is unit-testable (synchronous store calls,
// no async backend dependency) and would be better validated with a
// component-level vitest rather than a full Playwright E2E. The
// data-testid attributes added to ToolCallPendingApprovalContent in this
// branch enable that future vitest spec to find the elements.
test.describe.skip('MCP Tool Approval — optimistic UX', () => {
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
    await goToNewChatPage(page, testInfra.baseURL)

    const toolUseId = 'tu_approve_1'
    const messageId = 'msg_approve_1'

    // Two-call sequence: first call surfaces approval; second call resumes.
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_approve_1' }),
        mcpApprovalRequiredEvent({
          toolUseId,
          toolName: 'dangerous_op',
          server: 'mock-server',
          input: { target: 'production' },
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
      [
        startedEvent({ userMessageId: 'umsg_approve_resume', conversationId: messageId }),
        mcpToolStartEvent({ toolUseId, toolName: 'dangerous_op', server: 'mock-server' }),
        mcpToolCompleteEvent({ toolUseId, isError: false, result: { ok: true } }),
        textDeltaEvent({ delta: 'Done.' }),
        completeEvent(),
      ],
    ])

    await sendChatMessage(page, 'Please run the dangerous op')

    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`)
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.click('[data-testid="tool-approval-approve-once"]')

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
    await goToNewChatPage(page, testInfra.baseURL)
    const toolUseId = 'tu_approve_fail'

    // First call: approval required. Second call: HTTP 500 (simulates resume failure).
    let callIndex = 0
    await page.route(/\/api\/conversations\/[^/]+\/messages\/stream/, async route => {
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
    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`)
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.click('[data-testid="tool-approval-approve-once"]')

    // After the resume call fails, the optimistic update is reverted →
    // approval panel reappears.
    await expect(approval).toBeVisible({ timeout: 10000 })
  })

  test('deny: panel switches to denied state immediately (optimistic error status)', async ({
    page,
    testInfra,
  }) => {
    await goToNewChatPage(page, testInfra.baseURL)
    const toolUseId = 'tu_deny_1'

    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_deny_1' }),
        mcpApprovalRequiredEvent({
          toolUseId,
          toolName: 'unwanted_op',
          server: 'mock-server',
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

    await sendChatMessage(page, 'Try the unwanted op')
    const approval = page.locator(`[data-testid="tool-approval-${toolUseId}"]`)
    await expect(approval).toBeVisible({ timeout: 10000 })

    await page.click('[data-testid="tool-approval-deny"]')

    // Approval panel disappears (status flipped to 'error')
    await expect(approval).not.toBeVisible({ timeout: 5000 })

    // The denied tool shows the Failed card text
    await expect(page.locator('text=Failed').first()).toBeVisible({ timeout: 5000 })
  })

  test('approve-for-conversation triggers a POST to /mcp-settings with auto_approved_tools', async ({
    page,
    testInfra,
  }) => {
    await goToNewChatPage(page, testInfra.baseURL)
    const toolUseId = 'tu_approve_conv'
    const serverId = 'srv_approve_conv'

    // Capture all /mcp-settings PUTs
    const settingsBodies: unknown[] = []
    await page.route(/\/api\/conversations\/[^/]+\/mcp-settings/, async (route, req) => {
      if (req.method() === 'PUT') {
        try {
          settingsBodies.push(JSON.parse(req.postData() || '{}'))
        } catch {
          /* ignore */
        }
      }
      // We can't continue to real backend (conversation may not exist for
      // a new chat) — return 200.
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ success: true }),
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
    await expect(page.locator(`[data-testid="tool-approval-${toolUseId}"]`)).toBeVisible({
      timeout: 10000,
    })

    await page.click('[data-testid="tool-approval-approve-conv"]')

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

