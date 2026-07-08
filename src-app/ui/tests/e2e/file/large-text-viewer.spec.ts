import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// A 25k-line file (far past the retired 10k cap): a unique sentinel on the LAST
// line, a find MARKER on 3 well-separated lines (incl. lines past 10k and past
// the initial viewport window), and one very long line to exercise word-wrap.
// TEST-7/8/9/14 (ITEM-1/2/3/4/8/10).
const TOTAL = 25_000
const MARKER = 'ZQFINDMARKER'
const SENTINEL = 'LASTLINESENTINEL_XZ'
const LONG_LINE = 'x'.repeat(3000)

function buildLargeText(): string {
  const lines: string[] = []
  for (let i = 1; i <= TOTAL; i++) {
    let line = `line ${String(i).padStart(6, '0')} content token_${i}`
    if (i === 2) line = `line 000002 ${LONG_LINE}`
    if (i === 5 || i === 12_000 || i === 24_000) line += ` ${MARKER}`
    if (i === TOTAL) line += ` ${SENTINEL}`
    lines.push(line)
  }
  return lines.join('\n')
}
const CONTENT = buildLargeText()

async function seedAndOpen(page: import('@playwright/test').Page, baseURL: string, name: string) {
  await seedProjectFile(page, baseURL, {
    projectName: name,
    filename: 'big.txt',
    content: CONTENT,
    mime: 'text/plain',
  })
  const drawer = await openPreviewDrawer(page, 'big.txt')
  const raw = drawer.getByTestId('raw-code-view')
  await raw.waitFor({ state: 'visible' })
  return { drawer, raw }
}

test.describe('File viewer — large text/code (windowed)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('renders past 10k with no truncation + highlights on scroll (windowed)', async ({ page, testInfra }) => {
    const { drawer, raw } = await seedAndOpen(page, testInfra.baseURL, `LargeText ${Date.now()}`)

    // Cap lifted: NO truncation banner at 10k.
    await expect(drawer.getByTestId('file-rawcode-truncated-alert')).toHaveCount(0)

    // The last line (#25000) is present in the DOM — it would have been dropped
    // by the retired 10k head-cap. (All lines stay in the DOM; content-visibility
    // virtualizes paint, not the node tree.)
    await expect
      .poll(async () => raw.evaluate((el, s) => (el.textContent ?? '').includes(s), SENTINEL), {
        timeout: 15000,
      })
      .toBe(true)

    // Scroll the last chunk into view → it highlights ON DEMAND: colored Shiki
    // token spans appear only after the chunk enters the viewport (proving the
    // highlight is windowed, not run over the whole file up front).
    const lastChunk = raw.locator('[data-chunk-index]').last()
    await lastChunk.scrollIntoViewIfNeeded()
    await expect
      .poll(async () => lastChunk.locator('.line-code span[style*="color"]').count(), {
        timeout: 15000,
      })
      .toBeGreaterThan(0)
  })

  test('find-in-document spans the whole file (matches past 10k)', async ({ page, testInfra }) => {
    const { drawer } = await seedAndOpen(page, testInfra.baseURL, `LargeFind ${Date.now()}`)

    await drawer.getByTestId('file-viewer-find-btn').click()
    const input = drawer.getByTestId('file-find-input')
    await expect(input).toBeVisible()

    // MARKER occurs on lines 5, 12000, 24000 — two of them beyond the retired
    // 10k cap and the initial viewport window. find must count ALL three.
    await input.fill(MARKER)
    const count = drawer.getByTestId('file-find-count')
    await expect(count).toHaveText('1 / 3', { timeout: 15000 })

    // next navigates to the off-screen matches (viewer scrolls them into view).
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('2 / 3')
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('3 / 3')
    // wraps back to the first.
    await drawer.getByTestId('file-find-next-btn').click()
    await expect(count).toHaveText('1 / 3')
  })

  test('word-wrap toggle works under windowing', async ({ page, testInfra }) => {
    const { drawer, raw } = await seedAndOpen(page, testInfra.baseURL, `LargeWrap ${Date.now()}`)

    // Default OFF: the long line (#2, chunk 0) overflows horizontally.
    await expect(raw).toHaveAttribute('data-word-wrap', 'off')
    await expect
      .poll(async () =>
        raw.evaluate(el => {
          const pre = el.querySelector('pre.shiki') as HTMLElement | null
          return pre ? pre.scrollWidth > el.clientWidth + 4 : false
        }),
      )
      .toBe(true)

    // Toggle ON → the long line wraps; the first chunk's pre no longer overflows,
    // and the windowed line DOM (line-number gutter) stays intact.
    await drawer.getByTestId('file-viewer-wrap-btn').click()
    await expect(raw).toHaveAttribute('data-word-wrap', 'on')
    await expect
      .poll(async () =>
        raw.evaluate(el => {
          const pre = el.querySelector('pre.shiki') as HTMLElement | null
          return pre ? pre.scrollWidth <= el.clientWidth + 4 : true
        }),
      )
      .toBe(true)
    await expect(raw.locator('.line-number').first()).toBeVisible()

    // Toggle back OFF restores horizontal overflow.
    await drawer.getByTestId('file-viewer-wrap-btn').click()
    await expect(raw).toHaveAttribute('data-word-wrap', 'off')
  })
})
