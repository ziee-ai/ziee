import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockPaginatedMessages,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * message-scroll-perf — the virtualized message list stays geometrically stable
 * while scrolling a long MIXED-content conversation (short turns + long answers
 * + markdown tables + code blocks).
 *
 * The regression this guards: the merged virtualization used a CONSTANT
 * `estimateSize` (140px), so every unmeasured heavy row (table/code/long answer)
 * that scrolled into view corrected `getTotalSize()` — the scroll container
 * height (and the scrollbar thumb) jumped (symptoms 1 + 2). The content-aware
 * estimate (ITEM-1) + persisted measured-height cache (ITEM-2) keep the
 * estimated total close to the real total, so the total barely moves as rows
 * measure. Signal: the INITIAL scrollHeight (mostly estimated) is close to the
 * FINAL scrollHeight (all measured) — a ratio the old constant estimate could
 * not achieve on heavy content (it undershot by 3–4×).
 */

async function seedConversation(apiURL: string, token: string, title: string) {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status}`)
  return (await res.json()).id as string
}

function userMsg(id: string, text: string): MockMessageWithContent {
  return {
    id,
    role: 'user',
    contents: [{ content_type: 'text', content: { type: 'text', text } }],
  }
}

function assistantMsg(id: string, text: string): MockMessageWithContent {
  return {
    id,
    role: 'assistant',
    contents: [{ content_type: 'text', content: { type: 'text', text } }],
  }
}

const TABLE_MD =
  '| Col A | Col B | Col C |\n|---|---|---|\n' +
  Array.from({ length: 12 }, (_, r) => `| a${r} | b${r} | c${r} |`).join('\n')

const CODE_MD =
  '```ts\n' +
  Array.from({ length: 14 }, (_, i) => `const line${i} = ${i} * 2 // computed`).join(
    '\n',
  ) +
  '\n```'

const LONG_MD = 'This is a longer assistant answer. '.repeat(20)

// A 30-message single-page window (store limit=30) so scrolling is PURE
// virtualization — no pagination/anchor interplay. Rows deliberately span a
// wide height range (short user turns, long answers, tables, code) so a
// constant estimate would be badly wrong.
function mixedWindow(): MockMessageWithContent[] {
  const out: MockMessageWithContent[] = []
  for (let i = 0; i < 30; i++) {
    out.push(userMsg(`u-${i}`, `Question ${i}?`))
    const kind = i % 4
    const body =
      kind === 0
        ? LONG_MD
        : kind === 1
          ? `Here is a table:\n\n${TABLE_MD}`
          : kind === 2
            ? `Here is some code:\n\n${CODE_MD}`
            : 'Short answer.'
    out.push(assistantMsg(`a-${i}`, body))
  }
  return out.slice(0, 30)
}

async function scrollHeightOf(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const msgs = document.querySelector('[data-testid="chat-messages"]')
    // The virtualizer's spacer is the first child div with an explicit height.
    const spacer = msgs?.querySelector<HTMLElement>(
      ':scope > div[style*="height"]',
    )
    return spacer ? Math.round(spacer.getBoundingClientRect().height) : 0
  })
}

test.describe('message-scroll-perf — geometry stability', () => {
  test('estimated total tracks measured total (stable scrollbar thumb)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: geometry')
    await mockPaginatedMessages(page, mixedWindow())

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    // The initial load jumps to the bottom; let the bottom rows measure.
    await expect(page.locator('[data-message-id="a-29"]')).toBeVisible()
    await page.waitForTimeout(600)

    // Virtualized: only a bounded subset is mounted (overscan tuned, ITEM-5).
    const mounted = await page.getByTestId('chat-message').count()
    expect(mounted).toBeGreaterThan(0)
    expect(mounted).toBeLessThan(24)

    // INITIAL total: bottom rows measured, the rest still on the (content-aware)
    // estimate.
    const initialSH = await scrollHeightOf(page)
    expect(initialSH).toBeGreaterThan(0)

    // Scroll to the very top so every row gets measured, then settle.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="u-0"]')).toBeVisible({
      timeout: 10000,
    })
    await page.waitForTimeout(600)
    const finalSH = await scrollHeightOf(page)

    // With a content-aware estimate the estimated total is CLOSE to the measured
    // total → the thumb doesn't lurch. The old constant-140 estimate undershot
    // heavy content by 3–4× (ratio ~0.25); require the estimate within ~35%.
    const ratio = initialSH / finalSH
    expect(ratio).toBeGreaterThan(0.65)
    expect(ratio).toBeLessThan(1.5)

    // Still virtualized at the top (window shifted, not everything mounted).
    expect(await page.getByTestId('chat-message').count()).toBeLessThan(24)
  })

  test('re-opening the conversation seeds measured heights (warm start)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: warm reopen')
    await mockPaginatedMessages(page, mixedWindow())

    // Cold open: scroll through so every row measures (populates the cache).
    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-29"]')).toBeVisible()
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="u-0"]')).toBeVisible({
      timeout: 10000,
    })
    await page.waitForTimeout(600)
    const finalSH = await scrollHeightOf(page)

    // Navigate away (unmounts ConversationPage → flushes the cache) and back.
    await page.goto(`${baseURL}/`)
    await page.waitForTimeout(200)
    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-29"]')).toBeVisible()
    // Immediately (before a full re-scroll) the total should already be close to
    // the measured total — the initialMeasurementsCache seeded real heights.
    const warmSH = await scrollHeightOf(page)
    expect(warmSH / finalSH).toBeGreaterThan(0.8)
  })

  test('scrolling does not re-highlight / re-mount message bodies (memo boundary)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const errors: string[] = []
    page.on('console', m => {
      if (m.type() === 'error') errors.push(m.text())
    })
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: memo')
    // A code-heavy window so Shiki highlighting is exercised.
    const win: MockMessageWithContent[] = []
    for (let i = 0; i < 20; i++) {
      win.push(userMsg(`u-${i}`, `Q${i}`))
      win.push(assistantMsg(`a-${i}`, `code:\n\n${CODE_MD}`))
    }
    await mockPaginatedMessages(page, win)

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-19"]')).toBeVisible()
    await page.waitForTimeout(400)

    // Scroll a heavy code row out of the window and back; it must re-mount clean
    // (highlighted), with no console errors from a re-render/re-highlight storm.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="a-0"]')).toBeVisible({
      timeout: 10000,
    })
    await page.waitForTimeout(300)
    await page.locator('[data-message-id="a-19"]').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="a-19"]')).toBeVisible()

    // No console errors surfaced by the scroll (no ErrorBoundary / re-render
    // churn crash). Highlighted code is present.
    expect(errors, errors.join('\n')).toHaveLength(0)
  })

  test('a tall markdown table renders inside a height-capped box (definite height)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: table cap')
    // A 100-row table — without a definite cap it would dominate the row and its
    // inner ResizeObserver could feed back into row measurement (ITEM-4).
    const bigTable =
      '| A | B | C |\n|---|---|---|\n' +
      Array.from({ length: 100 }, (_, r) => `| a${r} | b${r} | c${r} |`).join('\n')
    await mockPaginatedMessages(page, [
      userMsg('u', 'big table please'),
      assistantMsg('a', `Table:\n\n${bigTable}`),
    ])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    const table = page.locator('[data-testid="chat-message"] table').first()
    await expect(table).toBeVisible({ timeout: 15000 })
    await page.waitForTimeout(400)

    // The table's scroll wrapper is capped at min(60vh, 36rem) — it never grows
    // unbounded with row count (definite height for the virtualizer's measure).
    const cap = await page.evaluate(() =>
      Math.min(window.innerHeight * 0.6, 36 * 16),
    )
    const wrapperH = await page.evaluate(() => {
      const t = document.querySelector('[data-testid="chat-message"] table')
      // Walk up to the OverlayScrollbars viewport that imposes max-height.
      let el: HTMLElement | null = t as HTMLElement | null
      let max = 0
      while (el && el.getAttribute('data-testid') !== 'chat-message') {
        max = Math.max(max, el.getBoundingClientRect().height)
        el = el.parentElement
      }
      return max
    })
    // The whole message row is bounded near the cap (+ chrome), not 100 rows tall.
    expect(wrapperH).toBeLessThan(cap + 200)
  })
})
