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
  // Ant Design Select with mode="multiple" renders as a combobox
  const tagFilter = page.getByRole('combobox', { name: /filter.*tag/i })
  await tagFilter.click()

  // Wait for dropdown to appear
  const dropdown = page.locator('.ant-select-dropdown:visible')
  await dropdown.waitFor({ state: 'visible' })

  for (const tag of tags) {
    // Click each option in the dropdown
    await page.locator('.ant-select-item-option', { hasText: new RegExp(`^${tag}$`, 'i') }).click()
  }

  // Close dropdown by pressing Escape or clicking outside
  await page.keyboard.press('Escape')
}

/**
 * Sort hub resources
 */
export async function sortHubResources(
  page: Page,
  sortBy: 'popular' | 'name' | 'size',
) {
  // Ant Design Select renders as a combobox
  // Use force: true because the selected value span intercepts pointer events
  const sortSelect = page.getByRole('combobox', { name: /sort/i })
  await sortSelect.click({ force: true })

  // Wait for dropdown to appear
  const dropdown = page.locator('.ant-select-dropdown').last()
  await dropdown.waitFor({ state: 'visible' })

  // Click the option matching the sortBy value
  const option = dropdown.locator('.ant-select-item-option', {
    hasText: new RegExp(`^${sortBy}$`, 'i')
  })
  await option.click()

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
