/**
 * PDF.js viewer — end-to-end interaction specs (TEST-8..12).
 *
 * Drives the REAL `PdfJsBody` component (real pdfjs-dist PDFViewer, real canvas
 * rendering, real text layer, real PDFFindController) through the backend-free
 * gallery: the `overlay-file-preview-drawer` surface renders the drawer with a
 * PDF-typed fixture, and the gallery mock-API serves the deterministic sample
 * PDF bytes for `/files/{id}/raw`. Only the byte-fetch boundary is mocked — the
 * raw endpoint itself is covered by the Rust integration test (TEST-1/2). The
 * fixture is a 3-page PDF whose page 1 contains the unique token "ZIEEFINDABLE".
 */
import { expect, test } from '@playwright/test'

const SURFACE = '/gallery.html?surface=overlay-file-preview-drawer&theme=light&accent=blue'

/** Open the drawer surface and wait for the first PDF.js canvas to render. */
async function openPdf(page: import('@playwright/test').Page) {
  await page.goto(SURFACE)
  await expect(page.getByTestId('file-pdf-toolbar')).toBeVisible({ timeout: 15000 })
  await expect(page.locator('.pdfViewer canvas').first()).toBeVisible({ timeout: 20000 })
}

// TEST-8 (ITEM-3,4,5,9,10): pdfjs renders, toolbar shown (not the office image
// body), a selectable text layer exists, no truncation banner.
test('TEST-8 pdf renders with toolbar + text layer, no truncation banner', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(String(e)))
  await openPdf(page)

  // The pdfjs toolbar (page nav / zoom / find), NOT the legacy image body.
  await expect(page.getByTestId('file-pdf-prev-page')).toBeVisible()
  await expect(page.getByTestId('file-pdf-zoom-in')).toBeVisible()
  await expect(page.getByTestId('file-pdf-find-toggle')).toBeVisible()

  // A text layer with real text spans => selectable/copyable (not image-only).
  await expect(page.locator('.pdfViewer .textLayer span').first()).toBeVisible({ timeout: 20000 })
  const spanCount = await page.locator('.pdfViewer .textLayer span').count()
  expect(spanCount).toBeGreaterThan(0)

  // The office-only "showing first N of M" banner must not appear for a real PDF.
  await expect(page.getByTestId('file-pdf-truncated-alert')).toHaveCount(0)
  expect(errors, `page errors: ${errors.join('; ')}`).toHaveLength(0)
})

// TEST-9 (ITEM-6): page navigation — indicator, prev/next, jump-to-page.
test('TEST-9 page navigation (indicator, next/prev, jump)', async ({ page }) => {
  await openPdf(page)
  await expect(page.getByTestId('file-pdf-page-indicator')).toContainText('of 3')
  await expect(page.getByTestId('file-pdf-page-input')).toHaveValue('1')

  await page.getByTestId('file-pdf-next-page').click()
  await expect(page.getByTestId('file-pdf-page-input')).toHaveValue('2')

  await page.getByTestId('file-pdf-prev-page').click()
  await expect(page.getByTestId('file-pdf-page-input')).toHaveValue('1')

  // Jump to page 3 via the input.
  await page.getByTestId('file-pdf-page-input').fill('3')
  await page.getByTestId('file-pdf-page-input').press('Enter')
  await expect(page.getByTestId('file-pdf-page-input')).toHaveValue('3')
})

// TEST-10 (ITEM-7): zoom in enlarges the rendered canvas; fit/actual work.
test('TEST-10 zoom changes the rendered canvas size', async ({ page }) => {
  await openPdf(page)
  const canvas = page.locator('.pdfViewer canvas').first()
  const before = await canvas.evaluate((c) => (c as HTMLCanvasElement).width)

  await page.getByTestId('file-pdf-zoom-in').click()
  await expect
    .poll(async () => canvas.evaluate((c) => (c as HTMLCanvasElement).width), { timeout: 10000 })
    .toBeGreaterThan(before)

  // fit-width + actual-size are accepted without error and keep a canvas rendered.
  await page.getByTestId('file-pdf-fit-width').click()
  await page.getByTestId('file-pdf-actual-size').click()
  await expect(canvas).toBeVisible()
})

// TEST-11 (ITEM-8): find highlights matches with an x-of-N count; miss = 0.
test('TEST-11 find in document (match count + highlight + no-match)', async ({ page }) => {
  await openPdf(page)
  await page.getByTestId('file-pdf-find-toggle').click()
  const findInput = page.getByTestId('file-pdf-find-input')
  await expect(findInput).toBeVisible()

  await findInput.fill('ZIEEFINDABLE')
  // PDFFindController reports the count via the updatefindmatchescount event.
  await expect(page.getByTestId('file-pdf-find-count')).toContainText('of 1', { timeout: 10000 })
  // A highlighted match appears in the text layer.
  await expect(page.locator('.pdfViewer .textLayer .highlight').first()).toBeVisible({ timeout: 10000 })

  // A term that isn't present resolves to zero matches.
  await findInput.fill('zznotpresentzz')
  await expect(page.getByTestId('file-pdf-find-count')).toContainText('0 of 0', { timeout: 10000 })
})

// TEST-12 (ITEM-11): the gallery renders the loaded PDF offline via the mockApi
// binary route with zero console errors / failed requests.
test('TEST-12 gallery renders PDF offline with no console errors', async ({ page }) => {
  const problems: string[] = []
  page.on('pageerror', (e) => problems.push(`pageerror: ${e}`))
  page.on('console', (m) => {
    if (m.type() === 'error') problems.push(`console: ${m.text()}`)
  })
  page.on('requestfailed', (r) => {
    const u = r.url()
    if (!/favicon|@vite|hot-update|__vite/.test(u)) problems.push(`requestfailed: ${u}`)
  })
  await openPdf(page)
  // Give async render a beat to surface any late error.
  await expect(page.locator('.pdfViewer canvas').first()).toBeVisible()
  await page.waitForTimeout(500)
  expect(problems, problems.join('\n')).toHaveLength(0)
})
