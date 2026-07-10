import { test, expect } from '../../fixtures/test-context'
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
  mockGetMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'

/**
 * run_js inner-tool approval (TEST-11/12/30/31).
 *
 * When a run_js script calls a GATED sub-tool it suspends IN-PROCESS and the
 * stream emits `runJsApprovalRequired`. Unlike the turn-boundary MCP approval,
 * this resolves via the SIDE-CHANNEL `POST /api/mcp/elicitation/{id}/respond`
 * (the same in-process oneshot ask_user uses) — so the stream stays open (no
 * `complete`) until the user answers. These specs drive the approve/deny path
 * through the real `JsToolApprovalContent` component and assert the resolve POST.
 */
test.describe('run_js inner-tool approval', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('approve: prompt resolves via side-channel /respond with accept', async ({
    page,
    testInfra,
  }) => {
    const eid = 'elic-runjs-approve'
    let respondAction: string | undefined
    await page.route('**/api/mcp/elicitation/*/respond', async (route) => {
      respondAction = route.request().postDataJSON()?.action
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' })
    })

    // Suspended stream: started + the approval frame, NO complete.
    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg-rj-approve' }),
        {
          event: 'runJsApprovalRequired',
          data: { elicitation_id: eid, tool_name: 'web_search', server: 'web_search', input: { query: 'x' } },
        },
      ],
    ])
    await mockGetMessages(page, [mockUserMessage({ id: 'umsg-rj-approve', text: 'run a script' })])

    await goToNewChatPage(page, testInfra.baseURL)
    await sendChatMessage(page, 'run a script')

    const prompt = page.locator(`[data-testid="run-js-approval-${eid}"]`).first()
    await expect(prompt).toBeVisible({ timeout: 30000 })

    await page.locator(`[data-testid="run-js-approval-approve-${eid}"]`).click()

    await expect.poll(() => respondAction, { timeout: 5000 }).toBe('accept')
    await expect(page.locator(`[data-testid="run-js-approval-status-${eid}"]`)).toHaveAttribute(
      'data-status',
      'approved',
      { timeout: 5000 },
    )
  })

  test('deny: prompt resolves via /respond with decline', async ({ page, testInfra }) => {
    const eid = 'elic-runjs-deny'
    let respondAction: string | undefined
    await page.route('**/api/mcp/elicitation/*/respond', async (route) => {
      respondAction = route.request().postDataJSON()?.action
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' })
    })

    await mockChatTokenStream(page, [
      [
        startedEvent({ userMessageId: 'umsg-rj-deny' }),
        {
          event: 'runJsApprovalRequired',
          data: { elicitation_id: eid, tool_name: 'web_search', server: 'web_search', input: {} },
        },
      ],
    ])
    await mockGetMessages(page, [mockUserMessage({ id: 'umsg-rj-deny', text: 'run a script' })])

    await goToNewChatPage(page, testInfra.baseURL)
    await sendChatMessage(page, 'run a script')

    await expect(page.locator(`[data-testid="run-js-approval-${eid}"]`).first()).toBeVisible({
      timeout: 30000,
    })
    await page.locator(`[data-testid="run-js-approval-deny-${eid}"]`).click()

    await expect.poll(() => respondAction, { timeout: 5000 }).toBe('decline')
    await expect(page.locator(`[data-testid="run-js-approval-status-${eid}"]`)).toHaveAttribute(
      'data-status',
      'denied',
      { timeout: 5000 },
    )
  })
})

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

async function sendChatMessage(page: import('@playwright/test').Page, text: string) {
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = page.locator('textarea[placeholder*="Type your message"]').first()
  await textarea.fill(text)
  await page.getByRole('button', { name: 'Send message' }).click()
}
