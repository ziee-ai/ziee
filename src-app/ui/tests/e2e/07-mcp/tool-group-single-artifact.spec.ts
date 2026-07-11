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
  completeEvent,
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'
import {
  seedAssistantWithToolResult,
  mockBackendFile,
} from '../chat/fixtures/mock-tool-result'

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

  test('a pending approval scrolls itself into view when it appears', async ({
    page,
    testInfra,
  }) => {
    // Spy on scrollIntoView (patched before any page script) recording the target
    // element's testid + behavior. This directly asserts the approval component
    // scrolled ITSELF into view on mount — robust against the chat's own
    // auto-scroll-to-bottom / reload confounders that make a viewport assertion
    // unreliable.
    await page.addInitScript(() => {
      ;(window as unknown as { __scrollTargets: unknown[] }).__scrollTargets = []
      const orig = Element.prototype.scrollIntoView
      Element.prototype.scrollIntoView = function (
        this: Element,
        arg?: boolean | ScrollIntoViewOptions,
      ) {
        ;(window as unknown as { __scrollTargets: unknown[] }).__scrollTargets.push({
          tid: this.getAttribute('data-testid'),
          behavior: typeof arg === 'object' ? arg.behavior : undefined,
        })
        return orig.call(this, arg as ScrollIntoViewOptions)
      }
    })

    const userMsgId = 'umsg_scroll'
    const useId = 'tu_scroll_1'
    const serverId = 'srv-scroll'

    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: userMsgId }),
        mcpApprovalRequiredEvent({
          toolUseId: useId,
          toolName: 'dangerous_op',
          server: serverId,
          input: { target: 'production' },
        }),
        completeEvent({ finishReason: 'tool_use' }),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: userMsgId, text: 'do the risky thing' }),
      {
        id: 'amsg_scroll',
        role: 'assistant',
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
    ])

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    await sendChatMessage(page, 'do the risky thing')

    const approval = page.locator(`[data-testid="tool-approval-${useId}"]`)
    await expect(approval).toBeVisible({ timeout: 15000 })

    // The approval element scrolled itself into view (smooth, since the test env
    // has no prefers-reduced-motion).
    await expect
      .poll(
        () =>
          page.evaluate(
            () =>
              (window as unknown as { __scrollTargets: { tid: string; behavior?: string }[] })
                .__scrollTargets,
          ),
        { timeout: 5000 },
      )
      .toContainEqual({ tid: `tool-approval-${useId}`, behavior: 'smooth' })
  })
})
