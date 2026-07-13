/**
 * chats-page-virtualization — the behavioural proof, against the backend-free
 * `?surface=seeded-conversation-list-long` gallery surface (≈200 conversations
 * driving the REAL `VirtualizedConversationList`).
 *
 * TEST-4  only a WINDOW of rows is in the DOM (not all 200)
 * TEST-5  scrolling UPDATES the window (top detaches, deep row attaches, back re-mounts)
 * TEST-6  no row-height jank — corrections settle to ~0 + totalSize stable after a pause
 *
 * Mirrors tests/e2e/visual/chat-scroll-stability.spec.ts (the MessageList analog).
 */
import { test, expect, type Page } from '@playwright/test'

const SURFACE =
  '/gallery.html?surface=seeded-conversation-list-long&theme=light&accent=blue'

const SCROLLER = 'chat-conversation-list-scroll'
const CARD = '[data-testid^="chat-conversation-card-g-conv-"]'

type Metrics = { corrections: number; reset: () => void; totalSize: () => number }
declare global {
  interface Window {
    __CHATLIST_METRICS__?: Metrics
  }
}

async function mountedCardCount(page: Page): Promise<number> {
  return page.locator(CARD).count()
}
async function corrections(page: Page): Promise<number> {
  return page.evaluate(() => window.__CHATLIST_METRICS__?.corrections ?? -1)
}
async function totalSize(page: Page): Promise<number> {
  return page.evaluate(() => window.__CHATLIST_METRICS__?.totalSize() ?? -1)
}

/** Set the scroll viewport's scrollTop and let the window settle. */
async function scrollTo(page: Page, top: number) {
  await page
    .getByTestId(SCROLLER)
    .evaluate((el, t) => (el.scrollTop = t), top)
  await page.waitForTimeout(200)
}

test.describe('chats-page-virtualization', () => {
  let consoleErrors: string[]
  let pageErrors: string[]
  test.beforeEach(async ({ page }) => {
    consoleErrors = []
    pageErrors = []
    page.on('console', m => {
      if (m.type() === 'error') consoleErrors.push(m.text())
    })
    page.on('pageerror', e => pageErrors.push(String(e)))
    await page.goto(SURFACE)
    // Wait for the seeded list to render its window + the footer to confirm all
    // 200 are loaded.
    await expect(page.getByText('Showing 200 of 200 conversations')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator(CARD).first()).toBeVisible()
  })

  test('TEST-4: only a WINDOW of rows is mounted (not all 200)', async ({
    page,
  }) => {
    const mounted = await mountedCardCount(page)
    // 200 rows exist logically; only the visible window (+ overscan) is in the
    // DOM. A generous ceiling that still proves windowing (600px box / ~76-96px
    // rows + overscan 8 ≈ 15-25 rows).
    expect(mounted).toBeGreaterThan(3)
    expect(mounted).toBeLessThan(40)
    // The total virtual height reflects ALL 200 rows (geometry preserved).
    expect(await totalSize(page)).toBeGreaterThan(200 * 60)
  })

  test('TEST-5: scrolling UPDATES the window', async ({ page }) => {
    const topId = 'chat-conversation-card-g-conv-0000'
    const deepId = 'chat-conversation-card-g-conv-0150'
    // Top row is mounted at rest; a deep row is NOT.
    await expect(page.getByTestId(topId)).toBeAttached()
    await expect(page.getByTestId(deepId)).toHaveCount(0)

    // Scroll far down → the top row detaches and the deep row mounts.
    await scrollTo(page, 150 * 90)
    await expect(page.getByTestId(topId)).toHaveCount(0)
    await expect(page.getByTestId(deepId)).toBeAttached()

    // Scroll back to the top → the top row re-mounts.
    await scrollTo(page, 0)
    await expect(page.getByTestId(topId)).toBeAttached()
  })

  test('TEST-6: no row-height jank — corrections settle after a scroll pause', async ({
    page,
  }) => {
    // Prime: scroll through once so rows measure, then reset the counter.
    await scrollTo(page, 4000)
    await scrollTo(page, 0)
    await page.evaluate(() => window.__CHATLIST_METRICS__?.reset())
    await page.waitForTimeout(200)

    // Scroll to a deep offset and PAUSE.
    await scrollTo(page, 8000)
    await page.waitForTimeout(700)
    const sizeAfterScroll = await totalSize(page)
    await page.waitForTimeout(700)

    // After the pause, the estimate was close enough that measured rows are not
    // still re-correcting the geometry: corrections settle low and totalSize is
    // stable across the pause (the scrollbar-thumb-jump signal is ~0).
    expect(await corrections(page)).toBeLessThanOrEqual(6)
    expect(await totalSize(page)).toBe(sizeAfterScroll)

    expect(consoleErrors, consoleErrors.join('\n')).toEqual([])
    expect(pageErrors, pageErrors.join('\n')).toEqual([])
  })
})
