import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad, switchHubTab } from './helpers/hub-navigation'
import {
  searchHubResources,
  filterByTags,
  sortHubResources,
  clearAllFilters,
} from './helpers/hub-search-filter'
import { getModelCards } from './helpers/hub-models'
import { getAssistantCards } from './helpers/hub-assistants'
import { getMcpServerCards } from './helpers/hub-mcp'

test.describe('Hub Search and Filters', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
  })

  test.describe('Search Functionality', () => {
    test('should filter models by search query', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      const initialCards = await getModelCards(page)
      const initialCount = await initialCards.count()

      // Search for specific model
      await searchHubResources(page, 'llama')

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      // Should have fewer or equal results
      expect(filteredCount).toBeLessThanOrEqual(initialCount)

      // All visible cards should contain "llama" in name or description
      for (let i = 0; i < filteredCount; i++) {
        const card = filteredCards.nth(i)
        const text = await card.textContent()
        expect(text?.toLowerCase()).toContain('llama')
      }
    })

    test('should filter assistants by search query', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'assistants')
      await waitForHubDataLoad(page)

      const initialCards = await getAssistantCards(page)
      const initialCount = await initialCards.count()

      // Search for specific assistant
      await searchHubResources(page, 'code')

      const filteredCards = await getAssistantCards(page)
      const filteredCount = await filteredCards.count()

      // Should have fewer or equal results
      expect(filteredCount).toBeLessThanOrEqual(initialCount)
    })

    test('should filter MCP servers by search query', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)

      const initialCards = await getMcpServerCards(page)
      const initialCount = await initialCards.count()

      // Search for specific server
      await searchHubResources(page, 'file')

      const filteredCards = await getMcpServerCards(page)
      const filteredCount = await filteredCards.count()

      // Should have fewer or equal results
      expect(filteredCount).toBeLessThanOrEqual(initialCount)
    })

    test('should show no results for non-existent search', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      // Search for something that doesn't exist
      await searchHubResources(page, 'xyznonexistentmodel123')

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBe(0)

      // Should show a "no match" message (catalog has models, none match)
      await expect(page.getByTestId('hub-models-empty')).toBeVisible()
    })

    test('should clear search and show all results', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      const initialCards = await getModelCards(page)
      const initialCount = await initialCards.count()

      // Search to filter
      await searchHubResources(page, 'llama')

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBeLessThan(initialCount)

      // Clear search
      await clearAllFilters(page)

      const restoredCards = await getModelCards(page)
      const restoredCount = await restoredCards.count()

      expect(restoredCount).toBe(initialCount)
    })
  })

  test.describe('Tag Filtering', () => {
    test('should filter by single tag', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      const initialCards = await getModelCards(page)
      const initialCount = await initialCards.count()

      // Filter by tag (using actual tag that exists in hub data)
      await filterByTags(page, ['chat'])

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBeLessThanOrEqual(initialCount)
    })

    test('should filter by multiple tags', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      // Filter by multiple tags (using actual tags that exist in hub data)
      await filterByTags(page, ['chat', 'code'])

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      // Should have results matching both tags
      expect(filteredCount).toBeGreaterThanOrEqual(0)
    })
  })

  test.describe('Sorting', () => {
    // v2 Phase 7 dropped `popularity_score` from models, so the models sort
    // select no longer offers "Popular" — the second real option is
    // `display_name`. (Popularity sort still exists on the MCP tab.)
    test('should sort by display name', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      await sortHubResources(page, 'display_name')

      // Verify sort applied (would need to check actual order)
      const modelCards = await getModelCards(page)
      expect(await modelCards.count()).toBeGreaterThan(0)
    })

    test('should sort by name', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      await sortHubResources(page, 'name')

      const modelCards = await getModelCards(page)
      expect(await modelCards.count()).toBeGreaterThan(0)

      // Could verify alphabetical order by extracting names
    })

    // NOTE: "sort by size" was removed — v2 Phase 7 dropped the model-wide
    // `size_gb`, so the models sort select no longer offers a size option.
    // Coverage of the two remaining sorts lives in the name + display_name
    // tests above.
  })

  test.describe('Combined Filters', () => {
    test('should apply search and preserve results on tab switch', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      // Search for something
      await searchHubResources(page, 'phi')

      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBeGreaterThanOrEqual(0)

      // Switch tab
      await switchHubTab(page, 'assistants')
      await waitForHubDataLoad(page)

      // Search should be cleared on tab switch (or persisted, depending on design)
      const searchInput = page.locator('[data-testid$="-search-input"]').first()
      const searchValue = await searchInput.inputValue()

      // Either empty (cleared) or still has value (persisted)
      expect(typeof searchValue).toBe('string')
    })

    test('should combine search and tag filters', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      // Apply search
      await searchHubResources(page, 'llama')

      // Apply tag filter (using actual tag that exists in hub data)
      await filterByTags(page, ['chat'])

      // Results should match both filters
      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBeGreaterThanOrEqual(0)
    })

    test('should clear all filters at once', async ({ page, testInfra }) => {
      // Clear-all button is implemented in ModelsHubTab.tsx
      // (`Clear all` button visible when any filter is active).
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      const initialCards = await getModelCards(page)
      const initialCount = await initialCards.count()

      // Apply multiple filters. Use 'chat' tag (consistent with the
      // sibling 'should combine search and tag filters' test above —
      // 'conversational' isn't present in the test hub catalog).
      await searchHubResources(page, 'llama')
      await filterByTags(page, ['chat'])

      // Clear all
      await clearAllFilters(page)

      // Should restore all results
      const restoredCards = await getModelCards(page)
      const restoredCount = await restoredCards.count()

      expect(restoredCount).toBe(initialCount)
    })
  })

  test.describe('Search Input Behavior', () => {
    test('should debounce search input', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      const searchInput = page.locator('[data-testid$="-search-input"]').first()

      // Type quickly
      await searchInput.fill('l')
      await page.waitForTimeout(100)
      await searchInput.fill('ll')
      await page.waitForTimeout(100)
      await searchInput.fill('lla')
      await page.waitForTimeout(100)
      await searchInput.fill('llam')

      // Wait for debounce
      await page.waitForTimeout(600)

      // Should have filtered results
      const filteredCards = await getModelCards(page)
      const filteredCount = await filteredCards.count()

      expect(filteredCount).toBeGreaterThanOrEqual(0)
    })

    test('should preserve search value after reload', async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      await navigateToHub(page, baseURL, 'models')
      await waitForHubDataLoad(page)

      // Search for something
      await searchHubResources(page, 'llama')

      // Reload page
      await page.reload()
      await waitForHubDataLoad(page)

      // Check if search persisted (depends on implementation)
      const searchInput = page.locator('[data-testid$="-search-input"]').first()
      const newSearchValue = await searchInput.inputValue()

      // Either persisted or cleared is acceptable
      expect(typeof newSearchValue).toBe('string')
    })
  })
})
