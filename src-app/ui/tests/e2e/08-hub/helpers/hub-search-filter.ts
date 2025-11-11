import { Page } from '@playwright/test'

/**
 * Search hub resources by name or description
 */
export async function searchHubResources(page: Page, query: string) {
  const searchInput = page.getByRole('textbox', { name: /search/i })
  await searchInput.clear()
  await searchInput.fill(query)
  // Wait for debounced search
  await page.waitForTimeout(500)
}

/**
 * Filter hub resources by tags
 */
export async function filterByTags(page: Page, tags: string[]) {
  const tagFilter = page.getByRole('button', { name: /filter.*tag/i })
  await tagFilter.click()

  const dropdown = page.getByRole('listbox')
  await dropdown.waitFor({ state: 'visible' })

  for (const tag of tags) {
    await page.getByRole('option', { name: new RegExp(tag, 'i') }).click()
  }

  // Close dropdown
  await page.keyboard.press('Escape')
}

/**
 * Sort hub resources
 */
export async function sortHubResources(
  page: Page,
  sortBy: 'popular' | 'name' | 'size',
) {
  const sortSelect = page.getByRole('combobox', { name: /sort/i })
  await sortSelect.selectOption({ label: new RegExp(sortBy, 'i') })
  // Wait for re-render
  await page.waitForTimeout(300)
}

/**
 * Clear all filters (search and tags)
 */
export async function clearAllFilters(page: Page) {
  // Clear search
  const searchInput = page.getByRole('textbox', { name: /search/i })
  await searchInput.clear()

  // Clear tag filters if clear button exists
  const clearButton = page.getByRole('button', { name: /clear.*filter/i })
  if (await clearButton.isVisible()) {
    await clearButton.click()
  }

  // Wait for filters to clear
  await page.waitForTimeout(500)
}
