/**
 * Large-file viewer gallery surfaces (file-viewer-virtualization) — the windowed
 * render paths against the backend-free gallery. TEST-13 (ITEM-9).
 *
 * Drives the ISOLATED `?surface=seeded-delimited-viewer-large` (>10k-row
 * DelimitedTable, row-virtualized, whole-set sort/filter) and
 * `?surface=seeded-rawcode-large` (thousands of lines, chunk-on-demand Shiki
 * highlight) surfaces — no overlay backdrops, no backend.
 */
import { test, expect, type Page } from '@playwright/test'

async function openSeeded(page: Page, slug: string) {
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
}

test.describe('Large-file viewer gallery surfaces', () => {
  let pageErrors: string[]
  test.beforeEach(async ({ page }) => {
    pageErrors = []
    page.on('pageerror', e => pageErrors.push(String(e)))
  })

  test('TEST-13a: large CSV mounts row-virtualized with the full row count', async ({ page }) => {
    await openSeeded(page, 'seeded-delimited-viewer-large')
    const table = page.getByTestId('file-delimited-table')
    await table.waitFor({ state: 'visible' })
    // No truncation banner (cap lifted); readout shows the full >10k count.
    await expect(page.getByTestId('file-delimited-truncated-alert')).toHaveCount(0)
    await expect(page.getByTestId('file-delimited-readout')).toContainText('12,000')
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-13b: large raw-code mounts windowed and highlights on view', async ({ page }) => {
    await openSeeded(page, 'seeded-rawcode-large')
    const raw = page.getByTestId('raw-code-view')
    await raw.waitFor({ state: 'visible' })
    // No truncation banner; the file is split into multiple chunk slots.
    await expect(page.getByTestId('file-rawcode-truncated-alert')).toHaveCount(0)
    await expect
      .poll(async () => raw.locator('[data-chunk-index]').count())
      .toBeGreaterThan(1)
    // The initially-visible chunks highlight (colored Shiki token spans).
    await expect
      .poll(async () => raw.locator('.line-code span[style*="color"]').count(), {
        timeout: 15000,
      })
      .toBeGreaterThan(0)
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })
})
