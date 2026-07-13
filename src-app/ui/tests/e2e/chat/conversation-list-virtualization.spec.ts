import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * chats-page-virtualization — the REAL /chats production path (chats-page
 * virtualization ITEM-5). Seeds conversations via the API, opens /chats, and
 * proves virtualization holds on the real page + composes with Load-More paging.
 *
 * TEST-7  per-row interactions survive virtualization (navigate + scroll-out-and-back)
 * TEST-8  the loaded set is windowed in the DOM (far fewer cards than loaded)
 *
 * Mirrors tests/e2e/chat/conversation-list-load-more.spec.ts seeding.
 */

const CARD = '[data-testid^="chat-conversation-card-"]'

/** Scroll the list's real scroll container (inner OverlayScrollbars viewport on
 *  desktop) by `delta`, found by walking up from a mounted card to the nearest
 *  scrollable ancestor. Robust to the exact DOM without a viewport testid. */
async function scrollListBy(page: Page, delta: number) {
  await page.locator(CARD).first().evaluate((el: HTMLElement, d: number) => {
    let n: HTMLElement | null = el.parentElement
    while (n) {
      const s = getComputedStyle(n)
      if (
        (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
        n.scrollHeight > n.clientHeight
      ) {
        n.scrollTop += d
        return
      }
      n = n.parentElement
    }
    window.scrollBy(0, d)
  }, delta)
  await page.waitForTimeout(350)
}

async function seedConversations(
  page: Page,
  apiURL: string,
  token: string,
  count: number,
  prefix: string,
) {
  for (let i = 0; i < count; i++) {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: `${prefix} ${String(i).padStart(3, '0')}` },
    })
    expect(res.ok()).toBeTruthy()
  }
}

test.describe('Chat — conversation list virtualization (real path)', () => {
  test('TEST-8: loaded set is windowed in the DOM (far fewer cards than loaded)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed 120 (6 pages at limit 20) so a large single set can be accumulated.
    await seedConversations(page, apiURL, token, 120, 'Virt')

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('domcontentloaded')

    // Load every page so all 120 are in the store.
    await expect(page.getByText(/Showing 20 of 120 conversations/)).toBeVisible({
      timeout: 30000,
    })
    for (let i = 0; i < 6; i++) {
      const loadMore = page.getByRole('button', { name: 'Load More' })
      if ((await loadMore.count()) === 0) break
      await loadMore.click()
      await page.waitForTimeout(300)
    }
    await expect(page.getByText(/Showing 120 of 120 conversations/)).toBeVisible({
      timeout: 30000,
    })

    // All 120 loaded, but only a bounded window is mounted in the DOM.
    const mounted = await page.locator(CARD).count()
    expect(mounted).toBeGreaterThan(3)
    expect(mounted).toBeLessThan(60)
  })

  test('TEST-7: per-row interactions survive virtualization', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Enough rows that virtualization is active (> a single viewport of cards).
    await seedConversations(page, apiURL, token, 40, 'RowInteract')

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('domcontentloaded')
    await expect(page.getByText(/Showing \d+ of 40 conversations/)).toBeVisible({
      timeout: 30000,
    })

    // A top row is mounted; capture its id + title, scroll it OUT of the window,
    // then back — it must re-mount with the SAME content (stable-key measurement,
    // no blank/pop-in row).
    const firstCard = page.locator(CARD).first()
    const firstId = await firstCard.getAttribute('data-testid')
    expect(firstId).toBeTruthy()
    const firstTitle = ((await firstCard.textContent()) ?? '').trim()
    expect(firstTitle).toContain('RowInteract')

    // Scroll the list far down (top row detaches) then back up (it re-mounts).
    await scrollListBy(page, 4000)
    await expect(page.locator(`[data-testid="${firstId}"]`)).toHaveCount(0)
    await scrollListBy(page, -8000)
    const back = page.locator(`[data-testid="${firstId}"]`)
    await expect(back).toBeVisible()
    expect((await back.textContent()) ?? '').toContain(
      firstTitle.slice(0, 'RowInteract 000'.length),
    )

    // Clicking a mounted card navigates to its conversation.
    await back.click()
    await expect(page).toHaveURL(/\/chat\//, { timeout: 15000 })
  })
})
