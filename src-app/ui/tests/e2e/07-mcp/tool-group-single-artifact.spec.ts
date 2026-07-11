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
  mcpApprovalRequiredEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'
import {
  seedAssistantWithToolResult,
  mockBackendFile,
} from '../chat/fixtures/mock-tool-result'
import type { MockMessageWithContent } from '../helpers/sse-mock-helpers'

/** A tall text message (for overflowing the virtualized list). */
function textMsg(id: string, role: 'user' | 'assistant', text: string): MockMessageWithContent {
  return { id, role, contents: [{ content_type: 'text', content: { type: 'text', text } }] }
}

/**
 * Follow-up: a SINGLE tool call that produces an artifact is now wrapped in the
 * same collapsible `McpToolGroupCard` (auto-opened) — not a bare card with the
 * files loose below it. A single tool with NO artifact stays the plain card. And
 * a pending approval that appears off-screen is scrolled into view.
 */
test.describe('MCP tool group — single-tool artifact wrapping + scroll-to-approval', () => {
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

  test('a single tool with ONE artifact is wrapped, auto-opened, and headed by the tool name', async ({
    page,
    testInfra,
  }) => {
    await mockBackendFile(page, {
      fileId: 'single-artifact-1',
      filename: 'data.csv',
      mimeType: 'text/csv',
      textContent: 'a,b\n1,2\n',
    })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      toolName: 'make_chart',
      resourceLinks: [
        {
          uri: '/api/files/single-artifact-1',
          name: 'data.csv',
          mime_type: 'text/csv',
          file_id: 'single-artifact-1',
        },
      ],
    })

    // Now wrapped in exactly one group card (was a bare card + loose files).
    const group = page.locator('[data-testid="mcp-toolgroup-card"]')
    await expect(group).toHaveCount(1)
    // Header shows the tool name, NOT "N tools called".
    await expect(group).toContainText('make_chart')
    await expect(group).not.toContainText('tools called')
    // Artifact visible INSIDE the card WITHOUT a click (auto-open on artifact).
    await expect(group.locator('[data-testid="tool-result-files"]')).toBeVisible({
      timeout: 10000,
    })
  })

  test('a single tool with MULTIPLE artifacts wraps and renders every file; a single tool with NO artifact stays a plain card', async ({
    page,
    testInfra,
  }) => {
    // Multiple artifacts → all inside one wrapper.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      toolName: 'render_all',
      resourceLinks: [
        { uri: '/api/files/multi-a/download', name: 'a.png', mime_type: 'image/png' },
        { uri: '/api/files/multi-b/download', name: 'b.png', mime_type: 'image/png' },
        { uri: '/api/files/multi-c/download', name: 'c.png', mime_type: 'image/png' },
      ],
    })
    const group = page.locator('[data-testid="mcp-toolgroup-card"]')
    await expect(group).toHaveCount(1)
    await expect(group.locator('[data-testid="inline-file-preview"]')).toHaveCount(3, {
      timeout: 10000,
    })

    // Fresh conversation: a single tool with NO artifact must NOT wrap.
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      toolName: 'noop_tool',
      resourceLinks: [],
    })
    const lastBubble = page
      .locator('[data-testid="chat-message"][data-role="assistant"]')
      .last()
    await expect(lastBubble.locator('[data-testid="mcp-toolgroup-card"]')).toHaveCount(0)
    await expect(
      lastBubble.locator(
        '[data-testid^="mcp-toolcall-card-"], [data-testid^="mcp-tooluse-card-"]',
      ),
    ).toHaveCount(1)
  })

  test('a pending approval below the fold is scrolled into view (user NOT at bottom)', async ({
    page,
    testInfra,
  }) => {
    // Reproduce the real scenario: a long answer where the user has scrolled up
    // (isAtBottom === false), then an approval streams at the tail — the app's
    // isAtBottom-gated auto-follow does NOT scroll to it, so the fix must force a
    // scroll-to-bottom via the virtualized list handle. We assert the EFFECT
    // (approval actually in the viewport), not that scrollIntoView was called.
    await page.setViewportSize({ width: 900, height: 600 })
    const useId = 'tu_appr_scroll'
    const serverId = 'srv-appr'
    // Turn 1: a very long answer that overflows the viewport. Turn 2: the approval.
    const longText = Array.from({ length: 140 }, (_, i) => `Line ${i + 1} of a very long streamed answer.`).join('\n')
    const turn1 = [
      mockUserMessage({ id: 'u1', text: 'tell me a lot' }),
      textMsg('a1', 'assistant', longText),
    ]
    const approvalTurn = [
      mockUserMessage({ id: 'u2', text: 'run it' }),
      {
        id: 'a2',
        role: 'assistant' as const,
        contents: [
          {
            content_type: 'tool_use',
            content: {
              type: 'tool_use',
              id: useId,
              name: 'dangerous_op',
              server_id: serverId,
              input: { target: 'production' },
            },
          },
        ],
      },
    ]

    // Two scripts — the 1st send consumes turn 1 (long text), the 2nd the approval.
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'u1' }),
        textDeltaEvent({ delta: longText, messageId: 'a1' }),
        completeEvent(),
      ],
      [
        startedEvent({ userMessageId: 'u2' }),
        mcpApprovalRequiredEvent({
          toolUseId: useId,
          toolName: 'dangerous_op',
          server: serverId,
          input: { target: 'production' },
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')

    // Turn 1: send + long answer → list overflows.
    await mockGetMessages(page, turn1)
    await sendChatMessage(page, 'tell me a lot')
    await expect(
      page.locator('[data-testid="chat-message"][data-role="assistant"]').last(),
    ).toBeVisible({ timeout: 15000 })

    const jumpBtn = page.getByTestId('chat-jump-to-latest-btn')

    // Scroll the message list to the top so the user is NOT at the bottom.
    await page.evaluate(() => {
      document.querySelectorAll<HTMLElement>('*').forEach(el => {
        if (el.scrollHeight > el.clientHeight + 8 && getComputedStyle(el).overflowY !== 'visible') {
          el.scrollTop = 0
        }
      })
      window.scrollTo(0, 0)
    })
    await expect(jumpBtn).toBeVisible() // isAtBottom === false (the pre-fix bug surface)

    // Turn 2: the reload keeps the long turn-1 history + the approval block, so the
    // approval stays below the fold unless the fix scrolls to it.
    await mockGetMessages(page, [...turn1, ...approvalTurn])
    await sendChatMessage(page, 'run it')

    // Without the fix the isAtBottom gate leaves the approval below the fold; the fix
    // force-scrolls the virtualized list to the tail → the approval is in the viewport.
    const approval = page.getByTestId(`tool-approval-${useId}`)
    await expect(approval).toBeVisible({ timeout: 15000 })
    await expect(approval).toBeInViewport({ timeout: 5000 })
  })
})
