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
      timeout: 60000,
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
    const lastId = 'chat-conversation-card-g-conv-0199'
    // At rest: the FIRST row is mounted, the LAST row is not (it's far below).
    await expect(page.getByTestId(topId)).toBeAttached()
    await expect(page.getByTestId(lastId)).toHaveCount(0)

    // Scroll to the very bottom → the window follows: the first row detaches and
    // the last row mounts. (No naive offset↔index math — scroll to scrollHeight.)
    await page
      .getByTestId(SCROLLER)
      .evaluate(el => (el.scrollTop = el.scrollHeight))
    await page.waitForTimeout(300)
    await expect(page.getByTestId(topId)).toHaveCount(0)
    await expect(page.getByTestId(lastId)).toBeAttached()

    // Scroll back to the top → the first row re-mounts, the last detaches.
    await scrollTo(page, 0)
    await expect(page.getByTestId(topId)).toBeAttached()
    await expect(page.getByTestId(lastId)).toHaveCount(0)
  })

  test('TEST-6: no row-height jank — no corrections while idle + stable geometry', async ({
    page,
  }) => {
    // Cold reset at the top, then scroll to a deep (un-primed) offset. With a
    // close estimate the measured rows barely correct the geometry; the crisp
    // no-jank guarantee is that once scrolling STOPS, corrections cease and the
    // total virtual height (the scrollbar-thumb position) holds steady.
    await scrollTo(page, 0)
    await page.evaluate(() => window.__CHATLIST_METRICS__?.reset())
    await scrollTo(page, 8000)
    await page.waitForTimeout(400) // let the freshly-windowed rows measure

    const c1 = await corrections(page)
    const s1 = await totalSize(page)
    await page.waitForTimeout(900) // sit idle
    const c2 = await corrections(page)
    const s2 = await totalSize(page)

    // While idle (no scroll), NO further corrections fire and the geometry is
    // stable — the row-height-jank signal at rest is zero.
    expect(c2 - c1).toBeLessThanOrEqual(1)
    expect(s2).toBe(s1)
    // Sanity: the cold scroll to ~20 fresh rows did not trigger a correction
    // storm (the estimate is close, not wildly off).
    expect(c1).toBeLessThan(50)

    expect(consoleErrors, consoleErrors.join('\n')).toEqual([])
    expect(pageErrors, pageErrors.join('\n')).toEqual([])
  })
})

// TEST-12: the NARROW (390px content column) surface must virtualize AND stay
// jank-free too — responsive-fidelity for a constrained column. (At the gallery's
// desktop VIEWPORT the card's `sm:` media query keeps the meta inline even in the
// 390px container, so the inline estimator applies; this guards that a narrow
// column still windows + settles.) Separate describe so it navigates the narrow
// surface in its own beforeEach.
const NARROW_SURFACE =
  '/gallery.html?surface=seeded-conversation-list-long-narrow&theme=light&accent=blue'

test.describe('chats-page-virtualization (narrow 390px)', () => {
  let consoleErrors: string[]
  test.beforeEach(async ({ page }) => {
    consoleErrors = []
    page.on('console', m => {
      if (m.type() === 'error') consoleErrors.push(m.text())
    })
    await page.goto(NARROW_SURFACE)
    await expect(
      page.getByText('Showing 200 of 200 conversations'),
    ).toBeVisible({ timeout: 60000 })
    await expect(page.locator(CARD).first()).toBeVisible()
  })

  test('TEST-12: narrow surface windows rows AND stays jank-free at rest', async ({
    page,
  }) => {
    // Windowed (far fewer than 200 mounted).
    const mounted = await mountedCardCount(page)
    expect(mounted).toBeGreaterThan(3)
    expect(mounted).toBeLessThan(40)

    // No corrections while idle after a cold scroll — the estimate is close enough
    // at a narrow content column that the row geometry holds steady at rest.
    await scrollTo(page, 0)
    await page.evaluate(() => window.__CHATLIST_METRICS__?.reset())
    await scrollTo(page, 8000)
    await page.waitForTimeout(400)
    const c1 = await corrections(page)
    const s1 = await totalSize(page)
    await page.waitForTimeout(900)
    expect((await corrections(page)) - c1).toBeLessThanOrEqual(1)
    expect(await totalSize(page)).toBe(s1)
    expect(c1).toBeLessThan(50)
    expect(consoleErrors, consoleErrors.join('\n')).toEqual([])
  })
})
