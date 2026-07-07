/**
 * kit <Table> actions — sort / filter / resize / column-chooser / numeric /
 * ellipsis / selection-copy / scroll-to-index — behavioral e2e against the
 * backend-free gallery. Lifecycle: .lifecycle/kit-table-actions (TEST-12..20).
 *
 * Driven through the `table-actions` + `table-scroll` gallery story cases
 * (src/dev/gallery/stories/data.story.tsx). No backend/login needed.
 */
import { test, expect, type Page, type Locator } from '@playwright/test'
import { openGallery } from './_gallery'

const T = 'g-table-actions'
const CASE = 'gallery-case-table-actions-actions'

function scopeOf(page: Page): Locator {
  return page.getByTestId(CASE)
}
async function rowOrder(scope: Locator): Promise<string[]> {
  return scope.locator(`[data-testid^="${T}-row-"]`).evaluateAll(els =>
    els.map(e => e.getAttribute('data-testid')!.replace(`${T}-row-`, '')),
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

  test('TEST-12: basic table unchanged + capability cases mount without errors', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    // The unchanged "basic" Table case still renders its rows.
    await expect(page.getByTestId('gallery-case-table-basic').getByTestId('g-table-row-1')).toBeVisible()
    // The new capability case mounts with rows.
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    await expect(scope.locator(`[data-testid^="${T}-row-"]`)).toHaveCount(3)
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
    expect(consoleErrors, consoleErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-13: sortable header cycles none→asc→desc→none with aria-sort', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    const qtyTh = scope.locator(`th:has([data-testid="${T}-sort-qty"])`)

    expect(await rowOrder(scope)).toEqual(['1', '2', '3']) // dataSource order
    await expect(qtyTh).toHaveAttribute('aria-sort', 'none')

    await scope.getByTestId(`${T}-sort-qty`).click() // asc: 2,10,30
    expect(await rowOrder(scope)).toEqual(['2', '1', '3'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'ascending')

    await scope.getByTestId(`${T}-sort-qty`).click() // desc: 30,10,2
    expect(await rowOrder(scope)).toEqual(['3', '1', '2'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'descending')

    await scope.getByTestId(`${T}-sort-qty`).click() // none → original
    expect(await rowOrder(scope)).toEqual(['1', '2', '3'])
    await expect(qtyTh).toHaveAttribute('aria-sort', 'none')
  })

  test('TEST-14: search filters rows case-insensitively; empty + clear', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    const search = scope.getByTestId(`${T}-search`)

    await search.fill('banana') // case-insensitive → Banana only
    expect(await rowOrder(scope)).toEqual(['1'])

    await search.fill('zzz') // no match → empty slot
    await expect(scope.getByTestId(`${T}-empty`)).toBeVisible()

    await search.fill('') // cleared → all rows back
    expect(await rowOrder(scope)).toEqual(['1', '2', '3'])
  })

  test('TEST-15: resize handle changes column width; double-click resets', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    const nameTh = scope.locator(`th:has([data-testid="${T}-sort-name"])`)
    const handle = scope.getByTestId(`${T}-resize-name`)

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
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    await scope.getByTestId(`${T}-columns-btn`).click()

    // Uncheck "note" → its sortable header disappears.
    await page.getByTestId(`${T}-col-toggle-note`).click()
    await expect(scope.getByTestId(`${T}-sort-note`)).toHaveCount(0)
    // Re-check → back.
    await page.getByTestId(`${T}-col-toggle-note`).click()
    await expect(scope.getByTestId(`${T}-sort-note`)).toHaveCount(1)

    // Hide two → the last visible toggle is disabled (guard).
    await page.getByTestId(`${T}-col-toggle-name`).click()
    await page.getByTestId(`${T}-col-toggle-qty`).click()
    await expect(page.getByTestId(`${T}-col-toggle-note`)).toBeDisabled()
  })

  test('TEST-17: numeric column right-aligned + tabular-nums; text column left', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    const row1 = scope.getByTestId(`${T}-row-1`)
    const qtyCell = row1.locator('td').nth(1)
    const nameCell = row1.locator('td').nth(0)
    expect(await qtyCell.evaluate(el => getComputedStyle(el).textAlign)).toBe('right')
    expect(await qtyCell.evaluate(el => getComputedStyle(el).fontVariantNumeric)).toContain('tabular-nums')
    expect(await nameCell.evaluate(el => getComputedStyle(el).textAlign)).toBe('left')
  })

  test('TEST-18: ellipsis cell is truncated + carries a title of the full value', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()
    const noteCell = scope.getByTestId(`${T}-row-2`).locator('td').nth(2)
    const title = await noteCell.getAttribute('title')
    expect(title).toContain('deliberately long cell value')
    expect(await noteCell.evaluate(el => el.className)).toContain('truncate')
  })

  test('TEST-19: cell + row selection, Ctrl/Cmd+C copies TSV to clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await openGallery(page, 'light', 'blue')
    const scope = scopeOf(page)
    await scope.scrollIntoViewIfNeeded()

    const nameCell = scope.getByTestId(`${T}-row-1`).locator('td').nth(0)
    await nameCell.click()
    await expect(nameCell).toHaveAttribute('data-selected', 'true')

    await page.keyboard.press('ControlOrMeta+c')
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Banana')
  })

  test('TEST-20: scrollToIndex scrolls a virtualized table to the target row', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    const scope = page.getByTestId('gallery-case-table-scroll-scroll')
    await scope.scrollIntoViewIfNeeded()
    // Row 400 is far down a 500-row virtualized list — not mounted initially.
    await expect(scope.getByTestId('g-table-scroll-row-400')).toHaveCount(0)
    await scope.getByTestId('g-table-scroll-btn').click()
    await expect(scope.getByTestId('g-table-scroll-row-400')).toBeVisible()
  })
})
