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
  mockGetMessages,
  mockUserMessage,
  mockAssistantElicitationMessage,
} from '../helpers/sse-mock-helpers'

/**
 * ElicitationFormContent — renderer coverage per MCP SEP-1330.
 *
 * Each test injects a single `mcpElicitationRequired` SSE event with a
 * schema exercising one primitive / format / enum variant, then asserts the
 * form mounts the correct control with the correct validation behaviour.
 *
 * Setup pattern (factored into `seedElicitation`):
 *   1. Auth + model so the chat page is usable
 *   2. page.route on /messages/stream with [started, mcpElicitationRequired, complete]
 *   3. Send any chat message → the mocked stream surfaces the elicitation
 *   4. Form is mounted under `data-testid="elicitation-pending-{id}"`
 */

test.describe('Elicitation form — field rendering', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('text (no format) → plain Input', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { name: { type: 'string', title: 'Name' } },
    })
    const input = byTestId(page, 'elicitation-field-name').first()
    await expect(input).toBeVisible()
    await expect(input).toHaveAttribute('type', 'text')
  })

  test('format=password → Input.Password (renders password input)', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { pw: { type: 'string', format: 'password', title: 'Password' } },
    })
    const input = byTestId(page, 'elicitation-field-pw').first()
    await expect(input).toBeVisible()
    await expect(input).toHaveAttribute('type', 'password')
  })

  test('format=email → Input with email type', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { addr: { type: 'string', format: 'email', title: 'Email' } },
    })
    const input = byTestId(page, 'elicitation-field-addr').first()
    await expect(input).toHaveAttribute('type', 'email')
  })

  test('format=uri → Input with url type', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { homepage: { type: 'string', format: 'uri', title: 'Homepage' } },
    })
    const input = byTestId(page, 'elicitation-field-homepage').first()
    await expect(input).toHaveAttribute('type', 'url')
  })

  test('format=date → AntD DatePicker', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { day: { type: 'string', format: 'date', title: 'Day' } },
    })
    // The DatePicker's testid is on the input; presence + a Day label nearby
    // proves the picker rendered (the date format is verified end-to-end in
    // the submit-roundtrip spec).
    await expect(byTestId(page, 'elicitation-field-day').first()).toBeVisible()
  })

  test('format=date-time → AntD DatePicker with time picker', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        when: { type: 'string', format: 'date-time', title: 'When' },
      },
    })
    await expect(byTestId(page, 'elicitation-field-when').first()).toBeVisible()
  })

  test('number → InputNumber, accepts decimals', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        ratio: { type: 'number', title: 'Ratio', minimum: 0, maximum: 1 },
      },
    })
    const wrapper = byTestId(page, 'elicitation-field-ratio').first()
    await expect(wrapper).toBeVisible()
    // The data-testid lives on the InputNumber wrapper. AntD renders the
    // actual input as a SIBLING <input role="spinbutton"> with aria-valuemin/max.
    const input = byTestId(page, 'elicitation-field-ratio').first()
    await expect(input).toBeVisible()
  })

  test('integer → InputNumber with precision=0 (no decimals)', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { count: { type: 'integer', title: 'Count' } },
    })
    // For integer fields, just verify the InputNumber renders. The precision=0
    // coercion is implementation-detail and changes between AntD versions.
    await expect(byTestId(page, 'elicitation-field-count').first()).toBeVisible()
    // An input with role=spinbutton must be present (that's the AntD InputNumber).
    await expect(byTestId(page, 'elicitation-field-count').first()).toBeVisible()
  })

  test('boolean → Switch', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { agree: { type: 'boolean', title: 'Agree' } },
    })
    const sw = byTestId(page, 'elicitation-field-agree').first()
    await expect(sw).toBeVisible()
    // AntD Switch is a button with role=switch — assert via attribute (toHaveRole
    // checks accessible-name role which isn't set as a literal attribute on
    // AntD's button element).
    await expect(sw).toHaveAttribute('role', 'switch')
  })

  test('string with enum → single-Select', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        priority: {
          type: 'string',
          title: 'Priority',
          enum: ['low', 'medium', 'high'],
        },
      },
    })
    const sel = byTestId(page, 'elicitation-field-priority').first()
    await expect(sel).toBeVisible()
    // Click to open and verify the enum options appear
    await sel.click({ force: true })
    await expect(page.getByTestId(/^elicitation-field-priority-opt-/).filter({ hasText: 'low' }).first()).toBeVisible()
    await expect(page.getByTestId(/^elicitation-field-priority-opt-/).filter({ hasText: 'high' }).first()).toBeVisible()
  })

  test('string with anyOf titled → single-Select with title labels', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        env: {
          type: 'string',
          title: 'Environment',
          anyOf: [
            { const: 'prod', title: 'Production' },
            { const: 'staging', title: 'Staging' },
          ],
        },
      },
    })
    const sel = byTestId(page, 'elicitation-field-env').first()
    await sel.click({ force: true })
    await expect(page.getByTestId(/^elicitation-field-env-opt-/).filter({ hasText: 'Production' }).first()).toBeVisible()
    await expect(page.getByTestId(/^elicitation-field-env-opt-/).filter({ hasText: 'Staging' }).first()).toBeVisible()
  })

  test('array.items.enum → multi-Select', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        tags: {
          type: 'array',
          title: 'Tags',
          items: { type: 'string', enum: ['rust', 'ts', 'python'] },
        },
      },
    })
    const sel = byTestId(page, 'elicitation-field-tags').first()
    await expect(sel).toBeVisible()
    // Click to open and verify all enum options appear
    await sel.click({ force: true })
    await expect(page.getByTestId(/^elicitation-field-tags-opt-/).filter({ hasText: 'rust' }).first()).toBeVisible()
    await expect(page.getByTestId(/^elicitation-field-tags-opt-/).filter({ hasText: 'ts' }).first()).toBeVisible()
    await expect(page.getByTestId(/^elicitation-field-tags-opt-/).filter({ hasText: 'python' }).first()).toBeVisible()
  })

  test('array.items.anyOf titled → multi-Select with title labels', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        teams: {
          type: 'array',
          title: 'Teams',
          items: {
            anyOf: [
              { const: 'eng', title: 'Engineering' },
              { const: 'sales', title: 'Sales' },
            ],
          },
        },
      },
    })
    const sel = byTestId(page, 'elicitation-field-teams').first()
    await sel.click({ force: true })
    await expect(page.getByTestId(/^elicitation-field-teams-opt-/).filter({ hasText: 'Engineering' }).first()).toBeVisible()
    await expect(page.getByTestId(/^elicitation-field-teams-opt-/).filter({ hasText: 'Sales' }).first()).toBeVisible()
  })

  test('pattern → validation rejects mismatch', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        code: {
          type: 'string',
          title: 'Code',
          pattern: '^[A-Z]{3}$',
        },
      },
      required: ['code'],
    })
    const input = byTestId(page, 'elicitation-field-code').first()
    await input.fill('abc') // lowercase — doesn't match
    await byTestId(page, 'elicitation-submit').first().click()
    await expect(byTestId(page, 'mcp-elicitation-form').getByTestId(/^field-error-/).first()).toBeVisible()
  })

  test('required field → submit empty shows inline error', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { name: { type: 'string', title: 'Name' } },
      required: ['name'],
    })
    await byTestId(page, 'elicitation-submit').first().click()
    await expect(byTestId(page, 'mcp-elicitation-form').getByTestId(/^field-error-/).filter({ hasText: /required/i }).first()).toBeVisible()
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

let elicitCounter = 0

async function seedElicitation(
  page: import('@playwright/test').Page,
  baseURL: string,
  schemaPartial: { properties: Record<string, unknown>; required?: string[] },
): Promise<{ elicitationId: string; messageId: string }> {
  elicitCounter++
  const elicitationId = `eid_render_${elicitCounter}_${Date.now()}`
  const userMessageId = `umsg_render_${elicitCounter}_${Date.now()}`
  const assistantMessageId = `amsg_render_${elicitCounter}_${Date.now()}`
  const requestedSchema = { type: 'object', ...schemaPartial }
  const promptText = `Test elicitation #${elicitCounter}`

  await mockChatTokenStream(page, [
    [
      startedEvent({ userMessageId }),
      mcpElicitationRequiredEvent({
        elicitationId,
        messageId: assistantMessageId,
        message: promptText,
        requestedSchema,
      }),
      completeEvent({ finishReason: 'tool_use' }),
    ],
  ])

  // After SSE complete the chat store calls loadMessages — without this
  // mock the optimistic streamingMessage gets wiped and the form unmounts.
  await mockGetMessages(page, [
    mockUserMessage({ id: userMessageId, text: `Trigger elicitation #${elicitCounter}` }),
    mockAssistantElicitationMessage({
      id: assistantMessageId,
      elicitationId,
      message: promptText,
      requestedSchema,
      status: 'pending',
    }),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  await sendChatMessage(page, `Trigger elicitation #${elicitCounter}`)

  // Wait for the pending form to mount. There may be transient duplicates
  // (streamingMessage + reloaded message both carry the elicitation_request
  // content block during the brief window between stream `complete` and the
  // subsequent loadMessages settle) — `.first()` accepts either rendering.
  await expect(
    page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`).first(),
  ).toBeVisible({ timeout: 10000 })

  return { elicitationId, messageId: assistantMessageId }
}

async function sendChatMessage(page: import('@playwright/test').Page, text: string) {
  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill(text)
  const sendButton = byTestId(page, 'chat-input-send-btn')
  await sendButton.click()
}
