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
 * Rich `ask_user` decision UX (the ziee-internal path, marked `x-ziee-askuser`):
 * per-option cards with descriptions, a recommended-first badge, an optional
 * preview, an always-available "Other" free-text escape, and a 1–4 question
 * Next/Back wizard. The chat stream + message reload are page.route-mocked so the
 * test isolates the UI; the backend marker-stamp + envelope are covered by the
 * Rust tiers.
 */

const OTHER = '__ziee_other__'

test.describe('ask_user rich decision UX', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  // TEST-9 — per-option cards + accept roundtrip.
  test('single-select renders option cards with descriptions; card select POSTs accept', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_cards_${Date.now()}`
    const pending = await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        format: {
          type: 'string',
          title: 'Which format?',
          enum: ['csv', 'json'],
          enumNames: ['CSV', 'JSON'],
          enumDescriptions: ['Spreadsheet-friendly', 'Nested + typed'],
        },
      },
      required: ['format'],
    })

    await expect(pending).toContainText('Assistant')
    // The per-option description text is visible on the card (not hidden in a dropdown).
    await expect(pending).toContainText('Spreadsheet-friendly')
    await expect(pending).toContainText('Nested + typed')
    // No wizard chrome for a single question.
    await expect(byTestId(page, 'elicitation-wizard-step')).toHaveCount(0)

    await byTestId(page, 'elicitation-field-format-opt-json').first().click()
    await byTestId(page, 'elicitation-submit').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })
    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('accept')
    expect((body.content as Record<string, unknown>).format).toBe('json')
  })

  // TEST-10 — recommended-first + badge.
  test('recommended option renders first and shows a Recommended badge', async ({
    page,
    testInfra,
  }) => {
    const elicitId = `eid_rec_${Date.now()}`
    const pending = await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        plan: {
          type: 'string',
          title: 'Plan?',
          enum: ['basic', 'pro'],
          'x-ziee-recommended': 'pro',
        },
      },
      required: ['plan'],
    })

    // The recommended badge is present on the recommended option.
    await expect(
      byTestId(page, 'elicitation-field-plan-opt-pro-recommended').first(),
    ).toBeVisible()
    // …and 'pro' is ordered before 'basic' in the DOM.
    const cards = pending.locator('[data-testid^="elicitation-field-plan-opt-"]')
    await expect(cards.first()).toHaveAttribute('data-testid', 'elicitation-field-plan-opt-pro')
  })

  // TEST-11 — the always-available Other escape.
  test('Other card reveals a text input; free-text POSTs as the answer', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_other_${Date.now()}`
    await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        color: { type: 'string', title: 'Color?', enum: ['red', 'green'] },
      },
      required: ['color'],
    })

    // Other is auto-offered; the input is hidden until it's chosen.
    await expect(byTestId(page, 'elicitation-field-color-other-input')).toHaveCount(0)
    await byTestId(page, `elicitation-field-color-opt-${OTHER}`).first().click()
    const otherInput = byTestId(page, 'elicitation-field-color-other-input').first()
    await expect(otherInput).toBeVisible()
    await otherInput.fill('chartreuse')

    await byTestId(page, 'elicitation-submit').first().click()
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('accept')
    // The sentinel never leaks — the typed value is the answer.
    expect((body.content as Record<string, unknown>).color).toBe('chartreuse')
  })

  // TEST-11b — Other selected but left blank is a blocking validation error.
  test('Other selected but blank blocks submit with a validation error', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_otherblank_${Date.now()}`
    const pending = await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        color: { type: 'string', title: 'Color?', enum: ['red', 'green'] },
      },
      required: ['color'],
    })

    await byTestId(page, `elicitation-field-color-opt-${OTHER}`).first().click()
    await expect(byTestId(page, 'elicitation-field-color-other-input').first()).toBeVisible()
    // Submit with the Other text still empty → blocked, error shown, no POST.
    await byTestId(page, 'elicitation-submit').first().click()
    await expect(pending.getByRole('alert')).toContainText('Other')
    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`),
    ).toHaveCount(0)
    expect(capture.count()).toBe(0)
  })

  // TEST-12b — multi-select (checkbox cards) roundtrip.
  test('multi-select checkbox cards POST an array of the chosen values', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_multi_${Date.now()}`
    await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        tags: {
          type: 'array',
          title: 'Which tags?',
          items: { enum: ['red', 'green', 'blue'] },
          minItems: 1,
        },
      },
      required: ['tags'],
    })

    await byTestId(page, 'elicitation-field-tags-opt-red').first().click()
    await byTestId(page, 'elicitation-field-tags-opt-blue').first().click()
    await byTestId(page, 'elicitation-submit').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })
    const body = capture.responses()[0].body as Record<string, unknown>
    expect(body.action).toBe('accept')
    expect((body.content as Record<string, unknown>).tags).toEqual(['red', 'blue'])
  })

  // TEST-12 — the Next/Back wizard for 2 questions + single final submit.
  test('two questions render a Next/Back wizard; single Submit returns both answers', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_wiz_${Date.now()}`
    await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        format: { type: 'string', title: 'Format?', enum: ['csv', 'json'] },
        compression: { type: 'string', title: 'Compression?', enum: ['none', 'zstd'] },
      },
      required: ['format', 'compression'],
    })

    // Step indicator + Back disabled on step 1; Next (not Submit) shown.
    await expect(byTestId(page, 'elicitation-wizard-step').first()).toContainText('Step 1 of 2')
    await expect(byTestId(page, 'elicitation-back')).toHaveCount(0)
    await expect(byTestId(page, 'elicitation-submit')).toHaveCount(0)

    // Next is blocked until the required step-1 choice is made.
    await byTestId(page, 'elicitation-next').first().click()
    await expect(byTestId(page, 'elicitation-wizard-step').first()).toContainText('Step 1 of 2')

    await byTestId(page, 'elicitation-field-format-opt-csv').first().click()
    await byTestId(page, 'elicitation-next').first().click()
    await expect(byTestId(page, 'elicitation-wizard-step').first()).toContainText('Step 2 of 2')

    // Back returns to step 1 preserving the choice.
    await byTestId(page, 'elicitation-back').first().click()
    await expect(byTestId(page, 'elicitation-wizard-step').first()).toContainText('Step 1 of 2')
    await expect(byTestId(page, 'elicitation-field-format-opt-csv').first()).toHaveAttribute(
      'data-selected',
      'true',
    )

    // Forward again, answer step 2, single Submit returns BOTH answers.
    await byTestId(page, 'elicitation-next').first().click()
    await byTestId(page, 'elicitation-field-compression-opt-zstd').first().click()
    await byTestId(page, 'elicitation-submit').first().click()

    await expect(
      page.locator(`[data-testid="elicitation-accepted-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })
    expect(capture.count()).toBe(1)
    const body = capture.responses()[0].body as Record<string, unknown>
    const content = body.content as Record<string, unknown>
    expect(content.format).toBe('csv')
    expect(content.compression).toBe('zstd')
  })

  // TEST-13 — decline is preserved on the wizard path.
  test('Decline on a wizard step POSTs decline and shows the declined card', async ({
    page,
    testInfra,
  }) => {
    const capture = await captureElicitationResponses(page)
    const elicitId = `eid_wizdecline_${Date.now()}`
    await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        a: { type: 'string', title: 'A?', enum: ['x', 'y'] },
        b: { type: 'string', title: 'B?', enum: ['m', 'n'] },
      },
      required: ['a', 'b'],
    })

    await byTestId(page, 'elicitation-decline').first().click()
    await expect(
      page.locator(`[data-testid="elicitation-declined-${elicitId}"]`).first(),
    ).toBeVisible({ timeout: 5000 })
    const declineBody = capture.responses()[0].body as Record<string, unknown>
    expect(declineBody.action).toBe('decline')
  })

  // TEST-14 — the optional per-option preview block.
  test('an option with a preview renders its monospace preview block', async ({
    page,
    testInfra,
  }) => {
    const elicitId = `eid_prev_${Date.now()}`
    const pending = await injectRich(page, testInfra.baseURL, elicitId, {
      'x-ziee-askuser': true,
      type: 'object',
      properties: {
        shape: {
          type: 'string',
          title: 'Shape?',
          enum: ['flat', 'nested'],
          enumPreviews: ['a,b\n1,2', null],
        },
      },
      required: ['shape'],
    })

    // The previewed option shows its block; the other option shows none.
    await expect(
      byTestId(page, 'elicitation-field-shape-opt-flat-preview').first(),
    ).toBeVisible()
    await expect(byTestId(page, 'elicitation-field-shape-opt-nested-preview')).toHaveCount(0)
    await expect(pending).toContainText('a,b')
  })
})

// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

/** Seed a rich (marker-stamped) ask_user elicitation and return the pending form. */
async function injectRich(
  page: Page,
  baseURL: string,
  elicitationId: string,
  requestedSchema: Record<string, unknown>,
): Promise<ReturnType<Page['locator']>> {
  const userMessageId = `umsg_${elicitationId}`
  const assistantMessageId = `amsg_${elicitationId}`
  const message = 'A couple of quick choices:'

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
    mockUserMessage({ id: userMessageId, text: 'help me export' }),
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
  await textarea.fill('help me export')
  await byTestId(page, 'chat-input-send-btn').click()

  const pending = page.locator(`[data-testid="elicitation-pending-${elicitationId}"]`).first()
  await expect(pending).toBeVisible({ timeout: 10000 })
  return pending
}
