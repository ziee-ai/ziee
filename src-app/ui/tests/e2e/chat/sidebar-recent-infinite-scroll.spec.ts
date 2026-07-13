import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * TEST-6..11 — sidebar "Recent chats" VIRTUALIZED infinite-scroll paging.
 *
 * The sidebar RecentConversationsWidget loads the first page on mount and
 * auto-loads the next page as the last virtual row nears the end; only a window
 * of rows is ever in the DOM (virtualization). Newest first, so with SBP-000
 * created first and SBP-044 last: SBP-044 is the top row, SBP-000 the oldest.
 */

const N = 45
const LIST = 'chat-recent-conversations-list'
const ROW = /^chat-recent-conversations-menu-item-/
const pad = (i: number) => `SBP-${String(i).padStart(3, '0')}`

async function seedConversations(apiURL: string, token: string, n: number) {
  for (let i = 0; i < n; i++) {
    const res = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ title: pad(i) }),
    })
    if (!res.ok) throw new Error(`seed ${i} failed: ${res.status}`)
  }
}

/** Scroll the sidebar list's OverlayScrollbars viewport to the bottom. */
async function scrollToBottom(page: Page) {
  return page.evaluate(
    listId => {
      const ul = document.querySelector(`[data-testid="${listId}"]`)
      let n = ul?.parentElement as HTMLElement | null
      while (n) {
        const s = getComputedStyle(n)
        if (
          (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
          n.scrollHeight > n.clientHeight
        ) {
          n.scrollTop = n.scrollHeight
          return n.scrollHeight
        }
        n = n.parentElement
      }
      return -1
    },
    LIST,
  )
}

async function domRowCount(page: Page) {
  return page.getByTestId(ROW).count()
}

test.describe('Sidebar recent chats — virtualized infinite scroll', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(apiURL)
    await seedConversations(apiURL, token, N)
    await loginAsAdmin(page, baseURL)
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(`${baseURL}/chats`)
    await expect(byTestId(page, LIST)).toBeVisible({ timeout: 30000 })
  })

  test('TEST-6/TEST-11: first page + virtualized window (not all rows in DOM)', async ({
    page,
  }) => {
    // Newest conversation is present on the first page.
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 1) }),
    ).toBeVisible()
    // The oldest is NOT rendered initially (page 1 only + windowed).
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toHaveCount(0)

    // TEST-11 — virtualization: the DOM holds only a window, materially fewer
    // than the 45 total (a non-virtualized list would render all loaded rows).
    const count = await domRowCount(page)
    expect(count).toBeGreaterThan(0)
    expect(count).toBeLessThan(30)

    // List semantics + position exposed for AT under virtualization.
    const list = byTestId(page, LIST)
    await expect(list).toHaveAttribute('role', 'list')
    const firstRow = list.locator('li').first()
    await expect(firstRow).toHaveAttribute('aria-posinset', /\d+/)
    await expect(firstRow).toHaveAttribute('aria-setsize', /\d+/)
  })

  test('TEST-7: scrolling auto-loads the next page with a loading indicator', async ({
    page,
  }) => {
    // Delay subsequent-page loads so the "Loading more" indicator is observable.
    await page.route('**/api/conversations?**', async route => {
      if (/[?&]page=(?:[2-9]|\d\d)\b/.test(route.request().url())) {
        await new Promise(r => setTimeout(r, 700))
      }
      await route.continue()
    })

    const pageTwo = page.waitForResponse(
      r => /\/api\/conversations\?[^ ]*page=2\b/.test(r.url()) && r.ok(),
    )
    await scrollToBottom(page)

    // The loading-more indicator shows during the (delayed) fetch...
    await expect(byTestId(page, 'chat-recent-loading-more')).toBeVisible({
      timeout: 5000,
    })
    await pageTwo // ...and the next page really is fetched on scroll (no button).
    await expect(byTestId(page, 'chat-recent-loading-more')).toHaveCount(0, {
      timeout: 5000,
    })

    // A page-2 conversation (SBP-024, the 21st-newest) is now reachable.
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 21) }),
    ).toBeVisible({ timeout: 10000 })
  })

  test('TEST-8: scrolling to the end reaches the oldest and then stops', async ({
    page,
  }) => {
    // Scroll page-by-page until the oldest row is reachable (≤ 6 scrolls covers
    // 3 pages with margin).
    for (let i = 0; i < 6; i++) {
      await scrollToBottom(page)
      await page.waitForTimeout(400)
      if (await page.getByTestId(ROW).filter({ hasText: pad(0) }).count()) break
    }
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toBeVisible({ timeout: 10000 })

    // End of list: no loading indicator, and a further scroll fires NO new fetch
    // (recentHasMore is false).
    await expect(byTestId(page, 'chat-recent-loading-more')).toHaveCount(0)
    let fetched = false
    await page.route('**/api/conversations?**', async route => {
      fetched = true
      await route.continue()
    })
    await scrollToBottom(page)
    await page.waitForTimeout(800)
    expect(fetched).toBe(false)
  })

  test('TEST-9: a new conversation appears at the top without resetting loaded pages', async ({
    page,
    testInfra,
  }) => {
    // Load a couple more pages first.
    await scrollToBottom(page)
    await page.waitForTimeout(500)
    await scrollToBottom(page)
    await page.waitForTimeout(500)
    // A page-2 row is loaded now.
    const olderRow = page.getByTestId(ROW).filter({ hasText: pad(N - 22) })
    await expect(olderRow).toBeVisible({ timeout: 10000 })

    // Create a brand-new conversation (cross-device path → SSE sync).
    const token = await getAdminToken(testInfra.apiURL)
    const res = await fetch(`${testInfra.apiURL}/api/conversations`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ title: 'SBP-NEWEST' }),
    })
    expect(res.ok).toBeTruthy()

    // It appears at the TOP of the sidebar…
    const newest = page.getByTestId(ROW).filter({ hasText: 'SBP-NEWEST' })
    await expect(newest).toBeVisible({ timeout: 15000 })
    // …and the previously-loaded older page was NOT dropped/reset.
    await scrollToBottom(page)
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 22) }),
    ).toBeVisible({ timeout: 10000 })
  })

  test('TEST-10: virtual rows keep menu-row fidelity (aria-current + row actions)', async ({
    page,
  }) => {
    // Selecting a row navigates and marks it current.
    const row = page.getByTestId(ROW).filter({ hasText: pad(N - 1) })
    await row.click()
    await expect(page).toHaveURL(/\/chat\//)
    await expect(row.locator('button').first()).toHaveAttribute(
      'aria-current',
      'page',
    )

    // Hover-reveal kebab + Delete still works under virtualization.
    const id = await row
      .locator('button')
      .first()
      .getAttribute('data-testid')
      .then(v => (v ?? '').replace('chat-recent-conversations-menu-item-', ''))
    await row.hover()
    const kebab = byTestId(page, `chat-recent-row-actions-btn-${id}`)
    await kebab.click()
    await page.getByRole('menuitem', { name: 'Delete' }).click()
    await byTestId(page, 'chat-conversation-delete-confirm-btn').click()
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 1) }),
    ).toHaveCount(0, { timeout: 10000 })
  })
})
