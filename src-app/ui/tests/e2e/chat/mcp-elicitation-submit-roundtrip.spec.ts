import type { Page } from '@playwright/test'
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
 * The elicitation date / date-time field is the kit <DatePicker> — a Button that
 * opens a Popover-hosted react-day-picker calendar, NOT a fillable text input
 * (so `.fill()` throws "not an <input>"). Pick a date by opening the calendar,
 * navigating from the currently-shown month (today) to the target month via the
 * prev/next nav buttons, then clicking the day cell (each carries
 * data-day="M/D/YYYY" in the en-US test locale).
 */
async function pickDateViaCalendar(
  page: Page,
  fieldTestId: string,
  year: number,
  month: number, // 1-12
  day: number,
) {
  await page.locator(`[data-testid="${fieldTestId}"]`).first().click()
  const popover = page.locator('[data-slot="popover-content"]')
  await expect(popover).toBeVisible({ timeout: 5000 })
  const now = new Date()
  const delta = (year - now.getFullYear()) * 12 + (month - 1 - now.getMonth())
  const navName = delta < 0 ? /previous month/i : /next month/i
  for (let i = 0; i < Math.abs(delta); i++) {
    await popover.getByRole('button', { name: navName }).click()
  }
  await popover.locator(`[data-day="${month}/${day}/${year}"]`).click()
}

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

test.describe('Elicitation form — submit / decline / cancel roundtrip', () => {
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

    await page.locator('[data-testid="elicitation-field-confirm"]').first().click()
    await page.locator('[data-testid="elicitation-submit"]').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
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

    await page.locator('[data-testid="elicitation-decline"]').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-declined-${elicitId}"]`).first(),
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

    await pickDateViaCalendar(page, 'elicitation-field-day', 2026, 5, 19)

    await page.locator('[data-testid="elicitation-submit"]').first().click()
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
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

    await pickDateViaCalendar(page, 'elicitation-field-when', 2026, 5, 19)

    await page.locator('[data-testid="elicitation-submit"]').first().click()
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })

    const body = capture.responses()[0].body as { content?: Record<string, string> }
    // ISO-8601 shape. The kit DatePicker is date-only (no time picker), so the
    // date-time field emits `yyyy-MM-dd'T'HH:mm:ss` with the time at T00:00:00
    // and NO timezone suffix (a documented limitation — see the FLAG in
    // ElicitationFormContent). Assert the ISO shape with an OPTIONAL zone.
    expect(body.content?.when).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?$/,
    )
    expect(body.content?.when).toContain('2026-05-19T00:00:00')
  })

  test('accept card displays submitted values', async ({ page, testInfra }) => {
    await captureElicitationResponses(page)
    const elicitId = `eid_show_${Date.now()}`
    await injectElicitation(page, testInfra.baseURL, elicitId, {
      properties: {
        nickname: { type: 'string', title: 'Nickname' },
      },
    })

    await page.locator('[data-testid="elicitation-field-nickname"]').first().fill('Phi')
    await page.locator('[data-testid="elicitation-submit"]').first().click()

    const accepted = page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first()
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
  const userMessageId = `umsg_${elicitationId}`
  const assistantMessageId = `amsg_${elicitationId}`
  const requestedSchema = { type: 'object', ...schemaPartial }

  await mockChatTokenStream(page, [
    [
      startedEvent({ userMessageId }),
      mcpElicitationRequiredEvent({
        elicitationId,
        messageId: assistantMessageId,
        message: 'Please fill this in',
        requestedSchema,
      }),
      completeEvent({ finishReason: 'tool_use' }),
    ],
  ])

  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: 'trigger' }),
    mockAssistantElicitationMessage({
      id: assistantMessageId,
      elicitationId,
      message: 'Please fill this in',
      requestedSchema,
      status: 'pending',
    }),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')

  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill('trigger')
  const sendButton = byTestId(page, 'chat-input-send-btn')
  await sendButton.click()

  await expect(
    page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`).first(),
  ).toBeVisible({ timeout: 10000 })
}
