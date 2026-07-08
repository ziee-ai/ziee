import * as XLSX from 'xlsx'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  seedProjectFile,
  seedProjectBinaryFile,
  openPreviewDrawer,
} from './helpers'

// TEST-10/11/12 (ITEM-5/6/7): the tabular + markdown viewers over LARGE data —
// full-dataset sort/filter past the retired 10k head-cap, and markdown's
// rendered-vs-raw boundary.

// ── CSV: 15k rows, a unique category on a row PAST the old 10k head-cap ───────
const CSV_ROWS = 15_000
const CSV_UNIQUE = 'UNIQUECAT9Z'
const CSV_UNIQUE_ROW = 14_000
function buildLargeCsv(): string {
  const lines = ['id,name,category,value']
  for (let i = 1; i <= CSV_ROWS; i++) {
    const category = i === CSV_UNIQUE_ROW ? CSV_UNIQUE : `cat-${i % 6}`
    lines.push(`${i},row-${i},${category},${i * 2}`)
  }
  return lines.join('\n')
}

// ── XLSX: 12k rows, a unique category on a row PAST the old 10k head-cap ──────
const XLSX_ROWS = 12_000
const XLSX_UNIQUE = 'XLSXUNIQUE7Q'
const XLSX_UNIQUE_ROW = 11_000
function buildLargeXlsxBytes(): number[] {
  const aoa: string[][] = [['id', 'name', 'category', 'value']]
  for (let i = 1; i <= XLSX_ROWS; i++) {
    const category = i === XLSX_UNIQUE_ROW ? XLSX_UNIQUE : `cat-${i % 6}`
    aoa.push([String(i), `row-${i}`, category, String(i * 2)])
  }
  const ws = XLSX.utils.aoa_to_sheet(aoa)
  const wb = XLSX.utils.book_new()
  XLSX.utils.book_append_sheet(wb, ws, 'Sheet1')
  const buf = XLSX.write(wb, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer
  return Array.from(new Uint8Array(buf))
}

// ── Markdown: a large doc for the rendered-vs-raw assessment ──────────────────
function buildLargeMarkdown(): string {
  const lines = ['# Big Doc Heading', '', 'Intro paragraph.', '']
  for (let i = 1; i <= 2000; i++) lines.push(`- list item ${i} with some content`)
  return lines.join('\n')
}

test.describe('File viewer — large tabular + markdown', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('CSV: full dataset, no truncation, filter spans rows past 10k', async ({ page, testInfra }) => {
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `LargeCsv ${Date.now()}`,
      filename: 'big.csv',
      content: buildLargeCsv(),
      mime: 'text/csv',
    })
    const drawer = await openPreviewDrawer(page, 'big.csv')
    await drawer.getByTestId('file-delimited-table').waitFor({ state: 'visible' })

    // Cap lifted: NO 10k truncation banner.
    await expect(drawer.getByTestId('file-delimited-truncated-alert')).toHaveCount(0)
    // Readout reflects the FULL row count (>10k).
    await expect(drawer.getByTestId('file-delimited-readout')).toContainText('15,000')

    // Filter for a category that ONLY exists on row 14,000 — past the old 10k
    // head-cap. It surfaces → sort/filter operate over the whole dataset.
    await drawer.getByTestId('file-delimited-table-search').fill(CSV_UNIQUE)
    await expect(drawer.getByTestId('file-delimited-readout')).toContainText('Showing 1 of 15,000')
    await expect(drawer.getByTestId('file-delimited-table').getByText(CSV_UNIQUE)).toBeVisible()
  })

  test('XLSX: full sheet, no truncation, filter spans rows past 10k', async ({ page, testInfra }) => {
    await seedProjectBinaryFile(page, testInfra.baseURL, {
      projectName: `LargeXlsx ${Date.now()}`,
      filename: 'big.xlsx',
      bytes: buildLargeXlsxBytes(),
      mime: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    })
    const drawer = await openPreviewDrawer(page, 'big.xlsx')
    const table = drawer.getByTestId('file-xlsx-table-Sheet1')
    await table.waitFor({ state: 'visible', timeout: 20000 })

    // Cap lifted from 10k → no per-sheet truncation banner at 10k.
    await expect(drawer.getByTestId('file-xlsx-truncated-alert-Sheet1')).toHaveCount(0)
    // Readout reflects the full parsed sheet (>10k).
    await expect(drawer.getByTestId('file-xlsx-Sheet1-readout')).toContainText('12,000')

    // Filter for a value that only exists on row 11,000 (past the old cap).
    await drawer.getByTestId('file-xlsx-table-Sheet1-search').fill(XLSX_UNIQUE)
    await expect(drawer.getByTestId('file-xlsx-Sheet1-readout')).toContainText('Showing 1 of 12,000')
    await expect(table.getByText(XLSX_UNIQUE)).toBeVisible()
  })

  test('Markdown: rendered keeps content; raw mode uses the windowed viewer', async ({ page, testInfra }) => {
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `LargeMd ${Date.now()}`,
      filename: 'big.md',
      content: buildLargeMarkdown(),
      mime: 'text/markdown',
    })
    const drawer = await openPreviewDrawer(page, 'big.md')

    // Rendered mode (default) shows the document — the heading renders.
    await expect(drawer.getByRole('heading', { name: 'Big Doc Heading' })).toBeVisible({
      timeout: 15000,
    })

    // Toggle to RAW mode → the raw view is the windowed RawCodeView (inherits the
    // chunk-on-demand highlight + lifted line cap).
    await drawer.getByTestId('file-viewer-raw-btn').click()
    await expect(drawer.getByTestId('raw-code-view')).toBeVisible()
  })
})
