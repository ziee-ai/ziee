import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * TEST-11 (feature: lazy-load-conversation-messages) — switching branches RESETS
 * the lazy-load window to the new branch's tail. After loading older pages on
 * branch A, activating branch B must drop A's older pages and show B's (shorter)
 * path from its tail.
 *
 * Branch list + per-branch message windows are mocked (the fork navigator +
 * store windowing + reset run for real); a mutable "active branch" flag flips
 * when the activate endpoint is hit so the subsequent tail load returns B.
 */

async function seedConversation(apiURL: string, token: string, title: string) {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status}`)
  return (await res.json()).id as string
}

function envelope(msgs: ReturnType<typeof mockUserMessage>[], start: number, end: number) {
  const full = msgs.map(m => ({
    id: m.id,
    role: m.role,
    contents: m.contents.map((c, idx) => ({
      id: `${m.id}-c${idx}`,
      message_id: m.id,
      content_type: c.content_type,
      content: c.content,
      sequence_order: idx,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    })),
    originated_from_id: '',
    edit_count: 0,
    created_at: new Date().toISOString(),
  }))
  const slice = full.slice(Math.max(0, start), end)
  return {
    messages: slice,
    has_more_before: start > 0,
    has_more_after: false,
  }
}

test.describe('Chat — lazy-load branch reset', () => {
  test('switching branch resets the window to the new branch tail', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Branch reset')

    const BRANCH_A = '00000000-0000-0000-0000-00000000000a'
    const BRANCH_B = '00000000-0000-0000-0000-00000000000b'
    // Branch A: 40 messages a-0..a-39 (fork at a-3). Branch B: 5 messages
    // a-0..a-2 (cloned) + b-3, b-4.
    const aMsgs = Array.from({ length: 40 }, (_, i) =>
      mockUserMessage({ id: `a-${i}`, text: `A message ${i}` }),
    )
    const bMsgs = [
      ...aMsgs.slice(0, 3),
      mockUserMessage({ id: 'b-3', text: 'B message 3' }),
      mockUserMessage({ id: 'b-4', text: 'B message 4' }),
    ]

    let activeBranch = BRANCH_A

    // Conversation GET reports the current active branch (drives the navigator).
    await page.route(
      new RegExp(`/api/conversations/${convId}(\\?|$)`),
      async (route, req) => {
        if (req.method() !== 'GET') return route.fallback()
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            id: convId,
            user_id: 'admin',
            title: 'Branch reset',
            active_branch_id: activeBranch,
            model_id: null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          }),
        })
      },
    )

    // Two branches sharing fork message a-3 so the navigator renders.
    await page.route(/\/api\/conversations\/[^/]+\/branches(\?|$)/, async (route, req) => {
      if (req.method() !== 'GET') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          {
            id: BRANCH_A,
            conversation_id: convId,
            parent_branch_id: null,
            created_from_message_id: null,
            fork_level: 'user',
            created_at: new Date(Date.now() - 1000).toISOString(),
          },
          {
            id: BRANCH_B,
            conversation_id: convId,
            parent_branch_id: BRANCH_A,
            created_from_message_id: 'a-3',
            fork_level: 'user',
            created_at: new Date().toISOString(),
          },
        ]),
      })
    })

    // Activate flips the active branch.
    await page.route(/\/api\/conversations\/[^/]+\/branches\/[^/]+\/activate$/, async route => {
      const m = route.request().url().match(/branches\/([^/]+)\/activate/)
      activeBranch = m?.[1] ?? activeBranch
      // Bare 204 (no JSON content-type) — a `application/json` header on an
      // empty body makes the api-client attempt `response.json()` and throw,
      // which would abort activateBranch before its loadMessages reset.
      await route.fulfill({ status: 204 })
    })

    // Paginated history per active branch.
    await page.route(/\/api\/conversations\/[^/]+\/messages(\?|$)/, async (route, req) => {
      if (req.method() !== 'GET') return route.fallback()
      const url = new URL(req.url())
      const before = url.searchParams.get('before')
      const limit = Number(url.searchParams.get('limit') ?? 30)
      const msgs = activeBranch === BRANCH_A ? aMsgs : bMsgs
      let start: number
      let end: number
      if (before) {
        const b = msgs.findIndex(m => m.id === before)
        start = Math.max(0, b - limit)
        end = b
      } else {
        start = Math.max(0, msgs.length - limit)
        end = msgs.length
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(envelope(msgs, start, end)),
      })
    })

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Branch A: tail loaded (a-39). Scroll up to load older pages, then force the
    // viewport to the very top so the oldest loaded message renders (under
    // virtualization a loaded-but-offscreen message isn't in the DOM).
    await expect(page.locator('[data-message-id="a-39"]')).toBeVisible()
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await page.waitForTimeout(600)
    await page.evaluate(() => {
      const list = document.querySelector('[data-testid="chat-messages"]')
      let n: HTMLElement | null = list?.parentElement ?? null
      while (n) {
        const s = getComputedStyle(n)
        if (
          (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
          n.scrollHeight > n.clientHeight
        ) {
          break
        }
        n = n.parentElement
      }
      ;(n ?? (document.scrollingElement as HTMLElement)).scrollTop = 0
    })
    await expect(page.locator('[data-message-id="a-0"]')).toBeVisible({ timeout: 10000 })

    // Switch to branch B via the navigator (next — A is the older parent, so
    // prev is disabled and next advances to B). B's tail (b-4) shows; A's
    // older/newer pages are gone (window reset).
    await expect(page.getByTestId('branch-navigator').first()).toBeVisible({ timeout: 10000 })
    await page.getByTestId('chat-branch-next-btn').first().click()

    await expect(page.locator('[data-message-id="b-4"]')).toBeVisible({ timeout: 10000 })
    await expect(page.locator('[data-message-id="a-39"]')).toHaveCount(0)
    await expect(page.locator('[data-message-id="a-10"]')).toHaveCount(0)
  })
})
