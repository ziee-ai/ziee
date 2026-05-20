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
  mcpElicitationRequiredEvent,
  completeEvent,
  captureElicitationResponses,
} from '../helpers/sse-mock-helpers'

/**
 * Elicitation form submit roundtrip — end-to-end UI flow:
 *
 *   chat input → mocked /messages/stream (emits mcpElicitationRequired) →
 *   form renders → user fills + Submit/Decline → POST /elicitation/{id}/respond
 *   is captured and validated → success/declined card replaces the form.
 *
 * All backend interaction goes through page.route mocks so we test the UI
 * layer in isolation from MCP server orchestration.
 */

// SKIPPED: same architectural mismatch as mcp-elicitation-form-rendering.spec.ts —
// page.route on /messages/stream is insufficient; the chat store's
// post-stream reload from the real backend wipes the elicitation content
// block. Backend coverage exists in
// src-app/server/tests/chat/mcp_elicitation_test.rs and
// src-app/server/tests/mcp/conformance_elicitation_test.rs (full e2e
// against a real MCP server fixture).
test.describe.skip('Elicitation form — submit / decline / cancel roundtrip', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('accept with valid form: POSTs accept+content, switches to success card', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_accept_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: { confirm: { type: 'boolean', title: 'Confirm' } },
    })

    await page.locator('[data-testid="elicitation-field-confirm"]').click()
    await page.click('[data-testid="elicitation-submit"]')

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`),
    ).toBeVisible({ timeout: 5000 })

    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('accept')
    expect((body.content as Record<string, unknown>).confirm).toBe(true)
  })

  test('decline: POSTs decline (no content) and switches to declined card', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_decline_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: { name: { type: 'string', title: 'Name' } },
    })

    await page.click('[data-testid="elicitation-decline"]')

    await expect(
      page.locator(`[data-testid="elicitation-declined-${elicitId}"]`),
    ).toBeVisible({ timeout: 5000 })

    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('decline')
    expect(body.content).toBeUndefined()
  })

  test('date field: dayjs value converts to YYYY-MM-DD ISO string in body', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_date_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: { day: { type: 'string', format: 'date', title: 'Day' } },
    })

    const picker = page.locator('[data-testid="elicitation-field-day"]')
    await picker.fill('2026-05-19')
    await page.keyboard.press('Enter')
    await page.waitForTimeout(300)

    await page.click('[data-testid="elicitation-submit"]')
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`),
    ).toBeVisible({ timeout: 5000 })

    const body = capture.responses()[0].body as { content?: Record<string, string> }
    expect(body.content?.day).toBe('2026-05-19')
  })

  test('date-time field: dayjs value converts to full ISO 8601 in body', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_datetime_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: { when: { type: 'string', format: 'date-time', title: 'When' } },
    })

    const picker = page.locator('[data-testid="elicitation-field-when"]')
    await picker.fill('2026-05-19 14:30:00')
    await page.keyboard.press('Enter')
    await page.waitForTimeout(300)

    await page.click('[data-testid="elicitation-submit"]')
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`),
    ).toBeVisible({ timeout: 5000 })

    const body = capture.responses()[0].body as { content?: Record<string, string> }
    // Full ISO 8601: 2026-05-19T14:30:00.000Z (or with offset — assert ISO-shape)
    expect(body.content?.when).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})$/)
  })

  test('accept card displays submitted values', async ({ page, testInfra }) => {
    await captureElicitationResponses(page)
    const elicitId = `eid_show_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: {
        nickname: { type: 'string', title: 'Nickname' },
      },
    })

    await page.locator('[data-testid="elicitation-field-nickname"]').fill('Phi')
    await page.click('[data-testid="elicitation-submit"]')

    const accepted = page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`)
    await expect(accepted).toBeVisible({ timeout: 5000 })
    await expect(accepted).toContainText('Phi')
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

async function injectElicitation(
  page: import('@playwright/test').Page,
  baseURL: string,
  elicitationId: string,
  schemaPartial: { properties: Record<string, unknown>; required?: string[] },
): Promise<void> {
  await mockChatStream(page, [
    [
      startedEvent({ userMessageId: `umsg_${elicitationId}` }),
      mcpElicitationRequiredEvent({
        elicitationId,
        messageId: `mid_${elicitationId}`,
        message: 'Please fill this in',
        requestedSchema: { type: 'object', ...schemaPartial },
      }),
      completeEvent({ finishReason: 'tool_use' }),
    ],
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')

  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill('trigger')
  const sendButton = page.getByRole('button', { name: 'Send message' })
  await sendButton.click()

  await expect(
    page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`),
  ).toBeVisible({ timeout: 10000 })
}
