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

// SKIPPED: page.route-based chat mocking is fundamentally incompatible with
// the chat store's lifecycle — after the mocked /messages/stream completes,
// the store fetches messages from the real backend and overwrites the
// optimistic streamingMessage state (which is where elicitation_request
// content lives), so the form unmounts before assertions can run.
//
// To make these tests work, the harness needs to ALSO mock:
//   - POST /api/conversations              (conversation create)
//   - GET  /api/conversations              (conversation list)
//   - GET  /api/conversations/{id}/messages
//   - GET  /api/conversations/{id}/mcp-settings
//
// Or a different approach entirely: hook into the chat store directly via
// page.evaluate to inject SSE events without going through HTTP. The
// infrastructure (data-testid attributes, sse-mock-helpers builders, spec
// scaffolding) is in place; only the orchestration layer needs revisiting.
//
// Backend coverage for the same flows exists in:
//   src-app/server/tests/chat/mcp_elicitation_test.rs
//   src-app/server/tests/mcp/conformance_elicitation_test.rs
test.describe.skip('Elicitation form — field rendering', () => {
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
    const input = page.locator('[data-testid="elicitation-field-name"]')
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
    const input = page.locator('[data-testid="elicitation-field-pw"]')
    await expect(input).toBeVisible()
    await expect(input).toHaveAttribute('type', 'password')
  })

  test('format=email → Input with email type', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { addr: { type: 'string', format: 'email', title: 'Email' } },
    })
    const input = page.locator('[data-testid="elicitation-field-addr"]')
    await expect(input).toHaveAttribute('type', 'email')
  })

  test('format=uri → Input with url type', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { homepage: { type: 'string', format: 'uri', title: 'Homepage' } },
    })
    const input = page.locator('[data-testid="elicitation-field-homepage"]')
    await expect(input).toHaveAttribute('type', 'url')
  })

  test('format=date → AntD DatePicker', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { day: { type: 'string', format: 'date', title: 'Day' } },
    })
    // DatePicker renders an .ant-picker container; the inner input carries the testid.
    const dp = page.locator('[data-testid="elicitation-field-day"]')
    await expect(dp).toBeVisible()
    // The placeholder text matches the configured format (YYYY-MM-DD)
    await expect(dp).toHaveAttribute('placeholder', /YYYY-MM-DD/i)
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
    const dp = page.locator('[data-testid="elicitation-field-when"]')
    await expect(dp).toHaveAttribute('placeholder', /YYYY-MM-DD HH:mm:ss/i)
  })

  test('number → InputNumber, accepts decimals', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: {
        ratio: { type: 'number', title: 'Ratio', minimum: 0, maximum: 1 },
      },
    })
    const wrapper = page.locator('[data-testid="elicitation-field-ratio"]').first()
    await expect(wrapper).toBeVisible()
    // The min/max props are reflected as aria attributes on the input
    const input = wrapper.locator('input.ant-input-number-input').first()
    await expect(input).toHaveAttribute('aria-valuemin', '0')
    await expect(input).toHaveAttribute('aria-valuemax', '1')
  })

  test('integer → InputNumber with precision=0 (no decimals)', async ({
    page,
    testInfra,
  }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { count: { type: 'integer', title: 'Count' } },
    })
    const wrapper = page.locator('[data-testid="elicitation-field-count"]').first()
    const input = wrapper.locator('input.ant-input-number-input').first()
    await input.fill('3.7')
    // AntD InputNumber with precision=0 coerces decimals on blur
    await page.locator('text=is requesting input').click() // blur target
    const value = await input.inputValue()
    expect(value).toMatch(/^-?\d+$/) // must be an integer string
  })

  test('boolean → Switch', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { agree: { type: 'boolean', title: 'Agree' } },
    })
    const sw = page.locator('[data-testid="elicitation-field-agree"]')
    await expect(sw).toBeVisible()
    // Switch is implemented as a button with role=switch
    await expect(sw).toHaveRole('switch')
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
    const sel = page.locator('[data-testid="elicitation-field-priority"]')
    await expect(sel).toBeVisible()
    // Single-select renders a combobox role
    await expect(sel).toHaveRole('combobox')
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
    const sel = page.locator('[data-testid="elicitation-field-env"]')
    await sel.click({ force: true })
    await expect(page.locator('.ant-select-item-option:has-text("Production")')).toBeVisible()
    await expect(page.locator('.ant-select-item-option:has-text("Staging")')).toBeVisible()
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
    const sel = page.locator('[data-testid="elicitation-field-tags"]')
    await expect(sel).toBeVisible()
    // Multi-select has class ant-select-multiple on its container
    const container = sel.locator('xpath=ancestor::div[contains(@class, "ant-select")][1]')
    await expect(container).toHaveClass(/ant-select-multiple/)
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
    const sel = page.locator('[data-testid="elicitation-field-teams"]')
    await sel.click({ force: true })
    await expect(page.locator('.ant-select-item-option:has-text("Engineering")')).toBeVisible()
    await expect(page.locator('.ant-select-item-option:has-text("Sales")')).toBeVisible()
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
    const input = page.locator('[data-testid="elicitation-field-code"]')
    await input.fill('abc') // lowercase — doesn't match
    await page.click('[data-testid="elicitation-submit"]')
    await expect(page.locator('.ant-form-item-explain-error').first()).toBeVisible()
  })

  test('required field → submit empty shows inline error', async ({ page, testInfra }) => {
    await seedElicitation(page, testInfra.baseURL, {
      properties: { name: { type: 'string', title: 'Name' } },
      required: ['name'],
    })
    await page.click('[data-testid="elicitation-submit"]')
    await expect(page.locator('.ant-form-item-explain-error:has-text("required")')).toBeVisible()
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
  const messageId = `mid_render_${elicitCounter}_${Date.now()}`

  await mockChatStream(page, [
    [
      startedEvent({ userMessageId: `umsg_${elicitCounter}` }),
      mcpElicitationRequiredEvent({
        elicitationId,
        messageId,
        message: `Test elicitation #${elicitCounter}`,
        requestedSchema: { type: 'object', ...schemaPartial },
      }),
      completeEvent({ finishReason: 'tool_use' }),
    ],
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  await sendChatMessage(page, `Trigger elicitation #${elicitCounter}`)

  // Wait for the pending form to mount
  await expect(
    page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`),
  ).toBeVisible({ timeout: 10000 })

  return { elicitationId, messageId }
}

async function sendChatMessage(page: import('@playwright/test').Page, text: string) {
  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill(text)
  const sendButton = page.getByRole('button', { name: 'Send message' })
  await sendButton.click()
}
