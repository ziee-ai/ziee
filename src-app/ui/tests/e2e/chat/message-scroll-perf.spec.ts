import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
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
 * Regression guarded: the merged virtualization used a CONSTANT `estimateSize`
 * (140px), so every unmeasured heavy row corrected `getTotalSize()` — the scroll
 * container height (and the scrollbar thumb) jumped (symptoms 1 + 2). The
 * content-aware estimate (ITEM-1) + persisted measured-height cache (ITEM-2) keep
 * the estimated total close to the real total. Signal: the INITIAL scrollHeight
 * (mostly estimated) is close to the FINAL scrollHeight (all measured) — a ratio
 * the old constant undershot by 3–4× on heavy content.
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

// Exactly 30 messages = 15 user/assistant pairs (u-0..u-14, a-0..a-14) → one
// store page (limit=30), has_more_before false → scrolling is PURE
// virtualization. Rows span a wide height range (short turns, long answers,
// tables, code) so a constant estimate would be badly wrong. Newest = a-14.
function mixedWindow(): MockMessageWithContent[] {
  const out: MockMessageWithContent[] = []
  for (let i = 0; i < 15; i++) {
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
  return out
}

// The virtualizer's spacer div carries the total scroll height (getTotalSize).
async function scrollHeightOf(page: Page): Promise<number> {
  return page.evaluate(() => {
    const msgs = document.querySelector('[data-testid="chat-messages"]')
    const spacer = msgs?.querySelector<HTMLElement>(
      ':scope > div[style*="height"]',
    )
    return spacer ? Math.round(spacer.getBoundingClientRect().height) : 0
  })
}

// Poll until the total height is stable across SEVERAL consecutive reads
// (deterministic across CI timing). Requiring a run of equal reads — not just
// one repeat — guards against a transient plateau between measurement bursts
// (e.g. async Shiki highlighting resizing a code row after a brief pause).
async function settledScrollHeight(page: Page): Promise<number> {
  let prev = -1
  let stable = 0
  for (let i = 0; i < 40; i++) {
    const h = await scrollHeightOf(page)
    if (h > 0 && h === prev) {
      if (++stable >= 3) return h
    } else {
      stable = 0
    }
    prev = h
    await page.waitForTimeout(120)
  }
  return prev
}

test.describe('message-scroll-perf — geometry stability', () => {
  test('estimated total tracks measured total (stable scrollbar thumb)', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: geometry')
    await mockPaginatedMessages(page, mixedWindow())

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    // Initial load jumps to the bottom; wait for the bottom rows to settle.
    await expect(page.locator('[data-message-id="a-14"]')).toBeVisible()
    const initialSH = await settledScrollHeight(page)
    expect(initialSH).toBeGreaterThan(0)

    // Virtualized: only a bounded subset is mounted (overscan tuned, ITEM-5).
    const mounted = await page.getByTestId('chat-message').count()
    expect(mounted).toBeGreaterThan(0)
    expect(mounted).toBeLessThan(24)

    // Scroll to the very top so every row gets measured, then settle.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="u-0"]')).toBeVisible({
      timeout: 10000,
    })
    const finalSH = await settledScrollHeight(page)

    // With a content-aware estimate the estimated total is CLOSE to the measured
    // total → the thumb doesn't lurch. The old constant-140 estimate undershot
    // heavy content by 3–4× (ratio ~0.25); require the estimate within ~35%.
    const ratio = initialSH / finalSH
    expect(ratio).toBeGreaterThan(0.65)
    expect(ratio).toBeLessThan(1.5)

    // Still virtualized at the top.
    expect(await page.getByTestId('chat-message').count()).toBeLessThan(24)
  })

  test('re-mounting the conversation seeds measured heights (warm start)', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: warm reopen')
    await mockPaginatedMessages(page, mixedWindow())

    // Cold open: scroll through so every row measures (populates the module
    // cache). The write-back is debounced, so wait for it to flush.
    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-14"]')).toBeVisible()
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="u-0"]')).toBeVisible({
      timeout: 10000,
    })
    const finalSH = await settledScrollHeight(page)
    await page.waitForTimeout(600) // let the debounced measured-height flush run

    // CLIENT-SIDE navigate away (New Chat pushes /chat, unmounting the
    // ConversationPage → its measured heights flush) then goBack() — a popstate
    // react-router handles WITHOUT a document reload, so the module-level cache
    // survives and the remounted ConversationPage seeds initialMeasurementsCache
    // from it. (A full page.goto reload would wipe the process-lifetime cache.)
    await page
      .getByTestId('layout-sidebar-primary-actions-menu')
      .getByText('New Chat')
      .click()
    await expect(page.getByTestId('new-chat-greeting')).toBeVisible({
      timeout: 15000,
    })
    await page.goBack()
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-14"]')).toBeVisible()

    // Immediately (before any re-measurement) the total must be ~EXACTLY the
    // measured total — a WORKING seed restores the real measured heights, so the
    // ratio is ~1.0. A broken/no-op seed would fall back to the estimator total
    // (materially off 1.0, per the sibling geometry test's 0.65..1.5 band), so a
    // tight bound here isolates the cache path from the estimator (FIX_ROUND-2).
    const warmSH = await scrollHeightOf(page)
    expect(warmSH / finalSH).toBeGreaterThan(0.95)
    expect(warmSH / finalSH).toBeLessThan(1.05)
  })

  test('code-heavy conversation scrolls without runtime errors', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // Only real app errors — filter benign infra noise so the assertion is not
    // flaky against unrelated console output.
    const IGNORE =
      /ResizeObserver loop|favicon|\[vite\]|net::ERR_|Failed to load resource|hydrat/i
    const errors: string[] = []
    page.on('console', m => {
      if (m.type() === 'error' && !IGNORE.test(m.text())) errors.push(m.text())
    })
    page.on('pageerror', e => errors.push(`pageerror: ${e.message}`))

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: code scroll')
    const win: MockMessageWithContent[] = []
    for (let i = 0; i < 15; i++) {
      win.push(userMsg(`u-${i}`, `Q${i}`))
      win.push(assistantMsg(`a-${i}`, `code:\n\n${CODE_MD}`))
    }
    await mockPaginatedMessages(page, win)

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.locator('[data-message-id="a-14"]')).toBeVisible()

    // Scroll up to the top and back down; heavy code rows re-window in and out.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="a-0"]')).toBeVisible({
      timeout: 10000,
    })
    // A Shiki-highlighted code block actually rendered (not a raw <pre> dump).
    await expect(page.locator('[data-message-id="a-0"] pre').first()).toBeVisible()
    await page.locator('[data-message-id="a-14"]').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="a-14"]')).toBeVisible()

    // No app-level console/page errors from the scroll (no re-render/re-highlight
    // crash, no ErrorBoundary). Locks the memo boundary against a runtime storm.
    expect(errors, errors.join('\n')).toHaveLength(0)
  })

  test('a tall markdown table renders inside a height-capped box (definite height)', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: table cap')
    const bigTable =
      '| A | B | C |\n|---|---|---|\n' +
      Array.from({ length: 100 }, (_, r) => `| a${r} | b${r} | c${r} |`).join('\n')
    await mockPaginatedMessages(page, [
      userMsg('u-0', 'big table please'),
      assistantMsg('a-0', `Table:\n\n${bigTable}`),
    ])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })
    const table = page.locator('[data-testid="chat-message"] table').first()
    await expect(table).toBeVisible({ timeout: 15000 })
    await settledScrollHeight(page)

    const cap = await page.evaluate(() =>
      Math.min(window.innerHeight * 0.6, 36 * 16),
    )
    const rowH = await page.evaluate(() => {
      const t = document.querySelector('[data-testid="chat-message"] table')
      let el: HTMLElement | null = t as HTMLElement | null
      let max = 0
      while (el && el.getAttribute('data-testid') !== 'chat-message') {
        max = Math.max(max, el.getBoundingClientRect().height)
        el = el.parentElement
      }
      return max
    })
    // The 100-row table is bounded near the cap (+ chrome), not 100 rows tall.
    expect(rowH).toBeLessThan(cap + 200)
  })
})
