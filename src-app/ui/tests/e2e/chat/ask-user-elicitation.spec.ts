import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatTokenStream,
  startedEvent,
  mcpElicitationRequiredEvent,
  completeEvent,
  captureElicitationResponses,
  mockGetMessages,
  mockUserMessage,
  mockAssistantElicitationMessage,
} from '../helpers/sse-mock-helpers'

/**
 * Built-in `ask_user` elicitation — UI flow for the ASSISTANT-initiated case.
 *
 * The generic elicitation renderer + submit roundtrip are already covered by
 * `mcp-elicitation-form-rendering.spec.ts` / `mcp-elicitation-submit-roundtrip.spec.ts`
 * (the form is server-name-agnostic). This spec pins the behaviour that's
 * specific to the `ask_user` tool: the form is labelled as the **Assistant**
 * (not a third-party MCP server) asking, and the headline multiple-choice
 * (enum) prompt round-trips the user's choice back through
 * `/elicitation/{id}/respond`.
 *
 * The chat stream + message reload are page.route-mocked so the test isolates
 * the UI layer — the backend roundtrip is covered by the Rust Tier-2
 * `elicitation_mcp_test.rs`.
 */

test.describe('ask_user elicitation — assistant-initiated form', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('multiple-choice: assistant-labelled form, choice POSTs accept+content', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_ask_accept_${Date.now()}`
    const pending = await injectAskUser(page, testInfra.baseURL, elicitId, {
      properties: {
        color: { type: 'string', title: 'Color', enum: ['red', 'green', 'blue'] },
      },
      required: ['color'],
    })

    // The form is attributed to the assistant, not a third-party MCP server.
    await expect(pending).toContainText('Assistant')
    await expect(pending).toContainText('is requesting input')

    // Rich rendering: enum options are directly-clickable radio CARDS (no
    // dropdown to open). Pick "green", then submit (1 question → no wizard).
    await byTestId(page, 'elicitation-field-color-opt-green').first().click()

    await byTestId(page, 'elicitation-submit').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })

    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('accept')
    expect((body.content as Record<string, unknown>).color).toBe('green')
  })

  test('decline: POSTs decline (no content) and switches to declined card', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_ask_decline_${Date.now()}`
    const pending = await injectAskUser(page, testInfra.baseURL, elicitId, {
      properties: {
        color: { type: 'string', title: 'Color', enum: ['red', 'green', 'blue'] },
      },
    })

    // Assistant-specific: the pending form is attributed to the Assistant.
    await expect(pending).toContainText('Assistant')

    await page.locator('[data-testid="elicitation-decline"]').first().click()

    const declined = page
      .locator(`[data-testid="elicitation-declined-${elicitId}"]`)
      .first()
    await expect(declined).toBeVisible({ timeout: 5000 })
    // The terminal declined card still attributes the Assistant (not a server).
    await expect(declined).toContainText('Assistant')

    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('decline')
    expect(body.content).toBeUndefined()
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

/**
 * Seed an assistant-initiated `ask_user` elicitation (server label "Assistant")
 * via mocked SSE + message reload, drive the chat send, and return the mounted
 * pending-form locator.
 */
async function injectAskUser(
  page: import('@playwright/test').Page,
  baseURL: string,
  elicitationId: string,
  schemaPartial: { properties: Record<string, unknown>; required?: string[] },
): Promise<ReturnType<import('@playwright/test').Page['locator']>> {
  const userMessageId = `umsg_${elicitationId}`
  const assistantMessageId = `amsg_${elicitationId}`
  // The real backend stamps `x-ziee-askuser` on every ask_user schema so the FE
  // renders the rich decision UX (option cards). Mirror that here.
  const requestedSchema = { 'x-ziee-askuser': true, type: 'object', ...schemaPartial }
  const message = 'Which color do you want?'

  await mockChatTokenStream(page, [
    [
      startedEvent({ userMessageId }),
      mcpElicitationRequiredEvent({
        elicitationId,
        messageId: assistantMessageId,
        message,
        requestedSchema,
        server: 'Assistant',
      }),
      completeEvent({ finishReason: 'tool_use' }),
    ],
  ])

  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'pick a color' }),
    mockAssistantElicitationMessage({
      id: assistantMessageId,
      elicitationId,
      message,
      requestedSchema,
      server: 'Assistant',
      status: 'pending',
    }),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')

  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill('pick a color')
  await byTestId(page, 'chat-input-send-btn').click()

  const pending = page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`).first()
  await expect(pending).toBeVisible({ timeout: 10000 })
  return pending
}
