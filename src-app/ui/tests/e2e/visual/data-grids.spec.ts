/**
 * Data grids (MCP tool-calls + memory audit log) gain kit-Table sort + filter.
 * Behavioral e2e against the backend-free gallery via the loaded seeded
 * surfaces. Lifecycle: .lifecycle/kit-table-actions (TEST-27..28).
 */
import { test, expect, type Locator, type Page } from '@playwright/test'

async function order(scope: Page | Locator, table: string): Promise<string[]> {
  return scope.locator(`[data-testid^="${table}-row-"]`).evaluateAll(
    (els, prefix) => els.map(e => e.getAttribute('data-testid')!.replace(prefix, '')),
    `${table}-row-`,
  )
}
async function openSeeded(page: Page, slug: string, table: string) {
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
  await page.getByTestId(table).waitFor({ state: 'visible' })
  // These grids fetch their rows from the (mock) store, so the table renders
  // empty for a tick before rows populate. Wait for the first row so an immediate
  // `order()` can't race the async load (a cold-start flake when this runs first).
  await page
    .locator(`[data-testid^="${table}-row-"]`)
    .first()
    .waitFor({ state: 'visible' })
}

test.describe('Data grids — sort + filter', () => {
  test('TEST-27: MCP tool-calls grid sorts by Duration; Duration is numeric (right-aligned)', async ({ page }) => {
    // Server-paginated grid → sort-only (client-side filter would mislead across
    // pages, DEC-5). Sort reorders the loaded page; Duration is a numeric column.
    const T = 'mcp-tool-calls-table'
    await openSeeded(page, 'seeded-mcp-tool-calls-loaded', T)
    // Rows id 1(search,120ms) 2(fetch,40ms) 3(remember,8ms).
    expect((await order(page, T)).sort()).toEqual(['1', '2', '3'])

    await page.getByTestId(`${T}-sort-duration_ms`).click() // asc: 8,40,120 → [3,2,1]
    expect(await order(page, T)).toEqual(['3', '2', '1'])

    // Duration column (5th: Time,Tool,Status,Source,Duration) is right-aligned.
    const durCell = page.getByTestId(`${T}-row-1`).locator('td').nth(4)
    expect(await durCell.evaluate(el => getComputedStyle(el).textAlign)).toBe('right')
  })

  test('TEST-28: memory audit grid sorts by Op + filters', async ({ page }) => {
    const T = 'memory-audit-table'
    await openSeeded(page, 'seeded-memory-audit-loaded', T)
    // id 1(ADD) 2(UPDATE) 3(DELETE).
    expect((await order(page, T)).sort()).toEqual(['1', '2', '3'])

    await page.getByTestId(`${T}-sort-op`).click() // asc: ADD,DELETE,UPDATE → [1,3,2]
    expect(await order(page, T)).toEqual(['1', '3', '2'])

    await page.getByTestId(`${T}-search`).fill('espresso') // snapshot 'Likes espresso' → id 1
    expect(await order(page, T)).toEqual(['1'])
  })
})
