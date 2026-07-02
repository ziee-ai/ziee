import { Page } from '@playwright/test'

/**
 * Selectors are tab-agnostic: only the active hub tab is mounted, so the
 * suffix-matched testid (`hub-<tab>-search-input`, `-sort-select`,
 * `-tags-multiselect`, `-clear-filters-btn`) resolves to a single element.
 */

/**
 * Search hub resources by name or description
 */
export async function searchHubResources(page: Page, query: string) {
  const searchInput = page.locator('[data-testid$="-search-input"]').first()
  await searchInput.clear()
  await searchInput.fill(query)
  // Wait for debounced search
  await page.waitForTimeout(500)
}

/**
 * Filter hub resources by tags. The kit MultiSelect opens a popover whose
 * options derive `${testid}-opt-${value}`.
 */
export async function filterByTags(page: Page, tags: string[]) {
  const tagFilter = page.locator('[data-testid$="-tags-multiselect"]').first()
  await tagFilter.click()

  for (const tag of tags) {
    await page
      .locator(`[data-testid$="-tags-multiselect-opt-${tag}"]`)
      .first()
      .click()
  }

  // Close popover
  await page.keyboard.press('Escape')
}

/**
 * Sort hub resources. The kit Select options derive `${testid}-opt-${value}`.
 */
export async function sortHubResources(
  page: Page,
  // v2 Phase 7 dropped `popularity_score` + `size_gb` from models/assistants,
  // so their sort selects expose only `name`/`display_name`; the MCP tab still
  // offers `popular` (aliased to name). Callers pass a value valid for the tab
  // they're on.
  sortBy: 'popular' | 'name' | 'size' | 'display_name',
) {
  const sortSelect = page
    .locator('[data-testid$="-sort-select"], [data-testid$="-sort-combobox"]')
    .first()
  await sortSelect.click()

  await page
    .locator(
      `[data-testid$="-sort-select-opt-${sortBy}"], [data-testid$="-sort-combobox-opt-${sortBy}"]`,
    )
    .first()
    .click()

  // Wait for re-render
  await page.waitForTimeout(300)
}

/**
 * Clear all filters (search and tags)
 */
export async function clearAllFilters(page: Page) {
  const searchInput = page.locator('[data-testid$="-search-input"]').first()
  await searchInput.clear()

  const clearButton = page
    .locator('[data-testid$="-clear-filters-btn"]')
    .first()
  if (await clearButton.isVisible().catch(() => false)) {
    await clearButton.click()
  }

  await page.waitForTimeout(500)
}
