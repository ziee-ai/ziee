/**
 * Tabular file viewer (CSV/TSV + XLSX) — sort / filter / export / jump-to-row /
 * copy / cell-expand — behavioral e2e against the backend-free gallery.
 * Lifecycle: .lifecycle/kit-table-actions (TEST-21..26).
 *
 * Drives the ISOLATED `?surface=seeded-delimited-viewer` / `seeded-xlsx-viewer`
 * surfaces (real DelimitedTable / XlsxSheet, no overlay backdrops).
 */
import { readFile } from 'node:fs/promises'
import { test, expect, type Page } from '@playwright/test'

const CSV = 'file-delimited-table'
const XLSX = 'file-xlsx-table-Sheet1'

async function openSeeded(page: Page, slug: string, table: string) {
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
  await page.getByTestId(table).waitFor({ state: 'visible' })
}
async function order(page: Page, table: string): Promise<string[]> {
  return page.locator(`[data-testid^="${table}-row-"]`).evaluateAll(
    (els, prefix) => els.map(e => e.getAttribute('data-testid')!.replace(prefix, '')),
    `${table}-row-`,
  )
}

test.describe('Tabular viewer', () => {
  let pageErrors: string[]
  test.beforeEach(async ({ page }) => {
    pageErrors = []
    page.on('pageerror', e => pageErrors.push(String(e)))
  })

  test('TEST-21: CSV sort + filter; # gutter is the first column', async ({ page }) => {
    await openSeeded(page, 'seeded-delimited-viewer', CSV)
    // 4 rows: Banana(0) apple(1) Cherry(2) Date(3); Qty = col '1' (10,2,30,7).
    expect(await order(page, CSV)).toEqual(['0', '1', '2', '3'])
    await page.getByTestId(`${CSV}-sort-1`).click() // Qty asc: 2,7,10,30
    expect(await order(page, CSV)).toEqual(['1', '3', '0', '2'])
    await page.getByTestId(`${CSV}-search`).fill('apple')
    expect(await order(page, CSV)).toEqual(['1'])
    const firstHeader = page.locator(`[data-testid="${CSV}"] thead th`).first()
    await expect(firstHeader).toContainText('#')
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-22: XLSX sheet exposes the same sort + filter', async ({ page }) => {
    await openSeeded(page, 'seeded-xlsx-viewer', XLSX)
    await page.getByTestId(`${XLSX}-sort-1`).click() // Qty asc: 2,10,30 → [1,0,2]
    expect(await order(page, XLSX)).toEqual(['1', '0', '2'])
    await page.getByTestId(`${XLSX}-search`).fill('Banana')
    expect(await order(page, XLSX)).toEqual(['0'])
  })

  test('TEST-23: Export view downloads only the filtered/sorted rows', async ({ page }) => {
    // The header-inclusive surface: DelimitedHeader (view-aware Export) over the
    // real DelimitedTable, coordinated via FileStore.fileTabularView.
    await openSeeded(page, 'seeded-delimited-viewer-shell', CSV)
    await page.getByTestId(`${CSV}-search`).fill('Banana') // 1 row
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByTestId('file-viewer-tabular-export-btn').click(),
    ])
    expect(download.suggestedFilename()).toBe('data-view.csv')
    const body = await readFile(await download.path(), 'utf8')
    expect(body).toContain('Name,Qty,Note')
    expect(body).toContain('Banana')
    expect(body).not.toContain('Cherry')
    expect(body).not.toContain('apple')
  })

  test('TEST-24: readout shows "X of Y rows" and updates with the filter', async ({ page }) => {
    await openSeeded(page, 'seeded-delimited-viewer', CSV)
    const readout = page.getByTestId('file-delimited-readout')
    await expect(readout).toHaveText('4 rows')
    await page.getByTestId(`${CSV}-search`).fill('Cherry')
    await expect(readout).toHaveText('Showing 1 of 4 rows')
    await page.getByTestId(`${CSV}-search`).fill('')
    await page.getByTestId('file-delimited-jump-input').fill('2')
    await page.getByTestId('file-delimited-jump-apply').click()
    await expect(page.getByTestId(`${CSV}-row-1`)).toBeVisible()
  })

  test('TEST-25: Copy button writes the selection as TSV to the clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await openSeeded(page, 'seeded-delimited-viewer-shell', CSV)
    // Select the Name cell (col index 1: # gutter, Name, Qty, Note) of row 0.
    await page.getByTestId(`${CSV}-row-0`).locator('td').nth(1).click()
    await page.getByTestId('file-viewer-tabular-copy-btn').click()
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Banana')
  })

  test('TEST-26: clicking a clipped cell opens a popover with the full value', async ({ page }) => {
    await openSeeded(page, 'seeded-delimited-viewer', CSV)
    await page.getByTestId('file-delimited-cell-2').click()
    await expect(page.getByTestId('file-delimited-cell-2-popover')).toBeVisible()
    await expect(page.getByTestId('file-delimited-cell-2-popover')).toContainText(
      'deliberately long cell value',
    )
  })
})
