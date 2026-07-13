import type { Page, Route, Request } from '@playwright/test'
import { byTestId } from '../testid'
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
  textDeltaEvent,
  completeEvent,
} from '../helpers/sse-mock-helpers'

/**
 * Streaming→persisted handoff — no flicker / no false empty notice (TEST-3).
 *
 * Reproduces the bug: on `complete`, the store used to delete the streaming
 * assistant row from `messages` and flip `isStreaming:false` BEFORE the persisted
 * tail was fetched back (an awaited `getHistory`). In that gap the answer
 * disappeared and the render-time empty-completion notice could flash.
 *
 * The fix fetches the persisted tail FIRST and swaps streaming→persisted in ONE
 * `set()`, so the assistant row is continuously present. This spec GATES the
 * `getHistory` response so the handoff window is held open deterministically:
 *   - FIXED: getHistory runs before teardown, so while it is blocked the streamed
 *     assistant row is still on screen (text visible, notice absent).
 *   - BASE (origin/khoi): teardown ran first, so while getHistory is blocked the
 *     assistant row is already deleted → the text assertion FAILS (revert-check).
 */

const STREAMED_TEXT = 'Hello from the stream, this is the answer.'

const assistantBubble = (page: Page) =>
  page.locator('[data-testid="chat-message"][data-role="assistant"]').last()

test.describe('Streaming→persisted handoff', () => {
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

  test('a normal answer stays visible across the handoff — no disappear, no empty notice', async ({
    page,
    testInfra,
  }) => {
    // Stream a normal text answer, then finalise.
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_hf_1' }),
        textDeltaEvent({ delta: STREAMED_TEXT, messageId: 'amsg_hf_1' }),
        completeEvent({ finishReason: 'end_turn' }),
      ],
    ])

    // Gated GET /messages: the first call (the post-`complete` reconcile) blocks
    // until we release it, holding the streaming→persisted handoff window open.
    let getHistoryCalls = 0
    let releaseFirst: () => void = () => {}
    const firstGate = new Promise<void>(resolve => {
      releaseFirst = resolve
    })
    const persisted = {
      messages: [
        {
          id: 'umsg_hf_1',
          role: 'user',
          contents: [
            {
              id: 'umsg_hf_1-c0',
              message_id: 'umsg_hf_1',
              content_type: 'text',
              content: { type: 'text', text: 'say hello' },
              sequence_order: 0,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ],
          originated_from_id: '',
          edit_count: 0,
          created_at: new Date().toISOString(),
        },
        {
          id: 'amsg_hf_1',
          role: 'assistant',
          contents: [
            {
              id: 'amsg_hf_1-c0',
              message_id: 'amsg_hf_1',
              content_type: 'text',
              content: { type: 'text', text: STREAMED_TEXT },
              sequence_order: 0,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ],
          originated_from_id: '',
          edit_count: 0,
          created_at: new Date().toISOString(),
        },
      ],
      has_more_before: false,
      has_more_after: false,
    }
    await page.route(
      /\/api\/conversations\/[^/]+\/messages(\?|$)/,
      async (route: Route, req: Request) => {
        if (req.method() !== 'GET') return route.fallback()
        getHistoryCalls += 1
        if (getHistoryCalls === 1) await firstGate
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(persisted),
        })
      },
    )

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')

    const textarea = byTestId(page, 'chat-message-textarea').first()
    await textarea.fill('say hello')
    await byTestId(page, 'chat-input-send-btn').click()

    // The streamed answer renders during streaming.
    await expect(assistantBubble(page)).toContainText('Hello from the stream', {
      timeout: 15000,
    })

    // Wait until the post-`complete` reconcile has been reached and is now BLOCKED
    // on our gate — the handoff window is open.
    await expect
      .poll(() => getHistoryCalls, { timeout: 15000 })
      .toBeGreaterThanOrEqual(1)

    // DURING the open handoff window: the assistant answer must still be on screen
    // (BASE deletes the row here → this fails) and the empty-completion notice must
    // NOT be showing.
    await expect(assistantBubble(page)).toContainText('Hello from the stream')
    await expect(byTestId(page, 'chat-empty-completion-notice')).toHaveCount(0)

    // Release the persisted swap; the finalized turn stays on screen, still no notice.
    releaseFirst()
    await expect(assistantBubble(page)).toContainText('Hello from the stream', {
      timeout: 15000,
    })
    await expect(byTestId(page, 'chat-empty-completion-notice')).toHaveCount(0)
  })
})
