/**
 * kit <Table> actions — sort / filter / resize / column-chooser / numeric /
 * ellipsis / selection-copy / scroll-to-index — behavioral e2e against the
 * backend-free gallery. Lifecycle: .lifecycle/kit-table-actions (TEST-12..20).
 *
 * Click-based tests drive the ISOLATED `?surface=seeded-kit-table-*` surfaces
 * (the browse-all canvas has open-overlay backdrops that intercept clicks);
 * assert-only checks use the browse view. Backend-free.
 */
import { test, expect, type Page, type Locator } from '@playwright/test'
import { openGallery } from './_gallery'

const T = 'g-table-actions'

async function openSeeded(page: Page, slug: string, table: string): Promise<Locator> {
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
  await page.getByTestId(table).waitFor({ state: 'visible' })
  return page.getByTestId(`${table}-root`)
}
async function rowOrder(scope: Page, table = T): Promise<string[]> {
  // The prefix is passed as an evaluateAll arg — closure vars aren't available
  // in the browser-side callback.
  return scope.locator(`[data-testid^="${table}-row-"]`).evaluateAll(
    (els, prefix) => els.map(e => e.getAttribute('data-testid')!.replace(prefix, '')),
    `${table}-row-`,
  )
}

test.describe('kit Table — actions', () => {
  let pageErrors: string[]
  let consoleErrors: string[]

  test.beforeEach(async ({ page }) => {
    pageErrors = []
    consoleErrors = []
    page.on('pageerror', e => pageErrors.push(String(e)))
    page.on('console', m => { if (m.type() === 'error') consoleErrors.push(m.text()) })
  })

  test('TEST-12: basic table unchanged + capability surface mounts without errors', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    await expect(page.getByTestId('gallery-case-table-basic').getByTestId('g-table-row-1')).toBeVisible()
    await openSeeded(page, 'seeded-kit-table-actions', T)
    expect(await rowOrder(page)).toEqual(['1', '2', '3'])
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
    expect(consoleErrors, consoleErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-13: sortable header cycles none→asc→desc→none with aria-sort', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const qtyTh = page.locator(`th:has([data-testid="${T}-sort-qty"])`)
    expect(await rowOrder(page)).toEqual(['1', '2', '3'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'none')

    await page.getByTestId(`${T}-sort-qty`).click() // asc: 2,10,30
    expect(await rowOrder(page)).toEqual(['2', '1', '3'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'ascending')

    await page.getByTestId(`${T}-sort-qty`).click() // desc: 30,10,2
    expect(await rowOrder(page)).toEqual(['3', '1', '2'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'descending')

    await page.getByTestId(`${T}-sort-qty`).click() // none
    expect(await rowOrder(page)).toEqual(['1', '2', '3'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'none')
  })

  test('TEST-14: search filters rows case-insensitively; empty + clear', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const search = page.getByTestId(`${T}-search`)
    await search.fill('banana')
    expect(await rowOrder(page)).toEqual(['1'])
    await search.fill('zzz')
    await expect(page.getByTestId(`${T}-empty`)).toBeVisible()
    await search.fill('')
    expect(await rowOrder(page)).toEqual(['1', '2', '3'])
  })

  test('TEST-15: resize handle changes column width; double-click resets', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const nameTh = page.locator(`th:has([data-testid="${T}-sort-name"])`)
    const handle = page.getByTestId(`${T}-resize-name`)
    const before = (await nameTh.boundingBox())!.width
    const hb = (await handle.boundingBox())!
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
    await page.mouse.down()
    await page.mouse.move(hb.x + hb.width / 2 + 90, hb.y + hb.height / 2, { steps: 6 })
    await page.mouse.up()
    const after = (await nameTh.boundingBox())!.width
    expect(after).toBeGreaterThan(before + 40)
    await handle.dblclick()
    const reset = (await nameTh.boundingBox())!.width
    expect(Math.abs(reset - before)).toBeLessThan(24)
  })

  test('TEST-16: column-chooser hides/shows; last visible column disabled', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    await page.getByTestId(`${T}-columns-btn`).click()
    await page.getByTestId(`${T}-col-toggle-note`).click()
    await expect(page.getByTestId(`${T}-sort-note`)).toHaveCount(0)
    await page.getByTestId(`${T}-col-toggle-note`).click()
    await expect(page.getByTestId(`${T}-sort-note`)).toHaveCount(1)
    await page.getByTestId(`${T}-col-toggle-name`).click()
    await page.getByTestId(`${T}-col-toggle-qty`).click()
    await expect(page.getByTestId(`${T}-col-toggle-note`)).toBeDisabled()
  })

  test('TEST-17: numeric column right-aligned + tabular-nums; text column left', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const row1 = page.getByTestId(`${T}-row-1`)
    const qtyCell = row1.locator('td').nth(1)
    const nameCell = row1.locator('td').nth(0)
    expect(await qtyCell.evaluate(el => getComputedStyle(el).textAlign)).toBe('right')
    expect(await qtyCell.evaluate(el => getComputedStyle(el).fontVariantNumeric)).toContain('tabular-nums')
    expect(await nameCell.evaluate(el => getComputedStyle(el).textAlign)).toBe('left')
  })

  test('TEST-18: ellipsis cell is truncated + carries a title of the full value', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const noteCell = page.getByTestId(`${T}-row-2`).locator('td').nth(2)
    expect(await noteCell.getAttribute('title')).toContain('deliberately long cell value')
    expect(await noteCell.evaluate(el => el.className)).toContain('truncate')
  })

  test('TEST-19: cell selection + Ctrl/Cmd+C copies TSV to clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await openSeeded(page, 'seeded-kit-table-actions', T)
    const nameCell = page.getByTestId(`${T}-row-1`).locator('td').nth(0)
    await nameCell.click()
    await expect(nameCell).toHaveAttribute('data-selected', 'true')
    await page.keyboard.press('ControlOrMeta+c')
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Banana')
  })

  test('TEST-20: scrollToIndex scrolls a virtualized table to the target row', async ({ page }) => {
    await openSeeded(page, 'seeded-kit-table-scroll', 'g-table-scroll')
    await expect(page.getByTestId('g-table-scroll-row-400')).toHaveCount(0)
    await page.getByTestId('g-table-scroll-btn').click()
    await expect(page.getByTestId('g-table-scroll-row-400')).toBeVisible()
  })
})
