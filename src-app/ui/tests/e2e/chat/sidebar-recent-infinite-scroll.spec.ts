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

/** Scroll the sidebar list viewport back to the top. */
async function scrollToTop(page: Page) {
  await page.evaluate(listId => {
    const ul = document.querySelector(`[data-testid="${listId}"]`)
    let n = ul?.parentElement as HTMLElement | null
    while (n) {
      const s = getComputedStyle(n)
      if (
        (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
        n.scrollHeight > n.clientHeight
      ) {
        n.scrollTop = 0
        return
      }
      n = n.parentElement
    }
  }, LIST)
}

/** Scroll to the bottom and wait for the next-page fetch it triggers to settle
 *  (deterministic — a fixed sleep races the fetch + re-measure cycle). Resolves
 *  after a page response or a short settle when there's nothing more to load. */
async function scrollStep(page: Page) {
  const resp = page
    .waitForResponse(r => /\/api\/conversations\?[^ ]*page=\d/.test(r.url()), {
      timeout: 5000,
    })
    .catch(() => null)
  await scrollToBottom(page)
  await resp
  await page.waitForTimeout(250)
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

  test('TEST-6: first page renders, older pages absent, list semantics present', async ({
    page,
  }) => {
    // Newest conversation is present on the first page.
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 1) }),
    ).toBeVisible()
    // The oldest is NOT in the DOM initially (it's neither loaded nor, if the
    // list eagerly filled a page or two, within the rendered window at the top).
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toHaveCount(0)
    // The DOM is windowed — never the full 45 at once.
    expect(await domRowCount(page)).toBeLessThan(N)

    // List semantics + position exposed for AT under virtualization.
    const list = byTestId(page, LIST)
    await expect(list).toHaveAttribute('role', 'list')
    const firstRow = list.locator('li').first()
    await expect(firstRow).toHaveAttribute('aria-posinset', /\d+/)
    await expect(firstRow).toHaveAttribute('aria-setsize', /\d+/)
  })

  test('TEST-11: virtualization windows the DOM — off-screen rows unmount', async ({
    page,
  }) => {
    // Scroll to the very bottom so ALL 45 rows are loaded.
    for (let i = 0; i < 12; i++) {
      if (await page.getByTestId(ROW).filter({ hasText: pad(0) }).count()) break
      await scrollStep(page)
    }
    // The oldest (bottom) row is now on-screen…
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toBeVisible({ timeout: 10000 })
    // …and the newest (top) row is UNMOUNTED — the decisive virtualization proof.
    // A non-virtualized list keeps all 45 rows in the DOM; a windowed one does not.
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(N - 1) }),
    ).toHaveCount(0)
    // The whole 45-item set is never all in the DOM at once.
    expect(await domRowCount(page)).toBeLessThan(N)
  })

  test('TEST-7: scrolling auto-loads the next page with a loading indicator', async ({
    page,
  }) => {
    // Delay every subsequent-page (>=2) load so the "Loading more" indicator is
    // observable, and apply it from a fresh mount (reload) so it covers the load.
    await page.route('**/api/conversations?**', async route => {
      if (/[?&]page=(?:[2-9]|\d\d)\b/.test(route.request().url())) {
        await new Promise(r => setTimeout(r, 800))
      }
      await route.continue()
    })
    await page.reload()
    await expect(byTestId(page, LIST)).toBeVisible({ timeout: 30000 })

    // Scrolling to the end triggers a (delayed) next-page load — with NO manual
    // button — and the loading-more indicator shows while it's in flight.
    await scrollToBottom(page)
    await expect(byTestId(page, 'chat-recent-loading-more')).toBeVisible({
      timeout: 8000,
    })

    // Scroll-driven paging reaches the deep (older) rows, and the indicator clears.
    for (let i = 0; i < 12; i++) {
      if (await page.getByTestId(ROW).filter({ hasText: pad(0) }).count()) break
      await scrollStep(page)
    }
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'chat-recent-loading-more')).toHaveCount(0, {
      timeout: 8000,
    })
  })

  test('TEST-8: scrolling to the end reaches the oldest and then stops', async ({
    page,
  }) => {
    // Scroll page-by-page until the oldest row is reachable.
    for (let i = 0; i < 12; i++) {
      if (await page.getByTestId(ROW).filter({ hasText: pad(0) }).count()) break
      await scrollStep(page)
    }
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toBeVisible({ timeout: 10000 })

    // End of list: no loading indicator, and further scrolling fires NO new fetch
    // (recentHasMore is false).
    await expect(byTestId(page, 'chat-recent-loading-more')).toHaveCount(0)
    let fetched = false
    await page.route('**/api/conversations?**', async route => {
      fetched = true
      await route.continue()
    })
    await scrollToBottom(page)
    await page.waitForTimeout(1000)
    await scrollToBottom(page)
    await page.waitForTimeout(1000)
    expect(fetched).toBe(false)
  })

  test('TEST-9: a new conversation appears at the top without resetting loaded pages', async ({
    page,
    testInfra,
  }) => {
    // Load a couple more pages first (scroll until a page-2 row exists).
    const olderRow = page.getByTestId(ROW).filter({ hasText: pad(N - 22) })
    for (let i = 0; i < 12; i++) {
      if (await olderRow.count()) break
      await scrollStep(page)
    }
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

    // It appears at the TOP of the sidebar (scroll back up — it's virtualized
    // away while we're scrolled to the bottom).
    const newest = page.getByTestId(ROW).filter({ hasText: 'SBP-NEWEST' })
    await expect(async () => {
      await scrollToTop(page)
      await expect(newest).toBeVisible({ timeout: 2000 })
    }).toPass({ timeout: 15000 })

    // …and the previously-loaded older page was NOT dropped/reset (scroll back
    // down and it still resolves).
    for (let i = 0; i < 12; i++) {
      if (await olderRow.count()) break
      await scrollStep(page)
    }
    await expect(olderRow).toBeVisible({ timeout: 10000 })
  })

  test('TEST-13: a persistent load-more failure does NOT hammer the API in a loop', async ({
    page,
  }) => {
    // Force every next-page (>=2) request to fail, and count them — applied from
    // a fresh mount (reload) so the load-more failure is exercised deterministically.
    let pageFetches = 0
    await page.route('**/api/conversations?**', async route => {
      if (/[?&]page=(?:[2-9]|\d\d)\b/.test(route.request().url())) {
        pageFetches++
        await route.fulfill({ status: 500, body: 'boom' })
        return
      }
      await route.continue()
    })
    await page.reload()
    await expect(byTestId(page, LIST)).toBeVisible({ timeout: 30000 })

    // Scroll to the bottom repeatedly; WITHOUT the failure gate the effect would
    // re-fire on every recentLoadingMore flip and issue dozens of requests.
    for (let i = 0; i < 6; i++) {
      await scrollToBottom(page)
      await page.waitForTimeout(500)
    }
    // The failed attempts are BOUNDED — no tight retry loop.
    expect(pageFetches).toBeGreaterThan(0)
    expect(pageFetches).toBeLessThanOrEqual(6)

    // A visible retry affordance is shown (recoverable even if the loaded page
    // fits the viewport and can't be scrolled). Restore the route, click Retry,
    // confirm the error clears and paging resumes to the oldest.
    await expect(byTestId(page, 'chat-recent-loadmore-error')).toBeVisible()
    await page.unroute('**/api/conversations?**')
    await byTestId(page, 'chat-recent-loadmore-retry').click()
    await expect(byTestId(page, 'chat-recent-loadmore-error')).toHaveCount(0, {
      timeout: 10000,
    })
    for (let i = 0; i < 12; i++) {
      if (await page.getByTestId(ROW).filter({ hasText: pad(0) }).count()) break
      await scrollStep(page)
    }
    await expect(
      page.getByTestId(ROW).filter({ hasText: pad(0) }),
    ).toBeVisible({ timeout: 10000 })
  })

  test('TEST-10: virtual rows keep menu-row fidelity (aria-current + row actions)', async ({
    page,
  }) => {
    // The row testid is ON the navigation <button> (MenuRowButton). Selecting it
    // navigates and marks it current.
    const row = page.getByTestId(ROW).filter({ hasText: pad(N - 1) })
    const id = (await row.getAttribute('data-testid'))!.replace(
      'chat-recent-conversations-menu-item-',
      '',
    )
    await row.click()
    await expect(page).toHaveURL(/\/chat\//)
    await expect(row).toHaveAttribute('aria-current', 'page')

    // Hover-reveal kebab + Delete still works under virtualization.
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
