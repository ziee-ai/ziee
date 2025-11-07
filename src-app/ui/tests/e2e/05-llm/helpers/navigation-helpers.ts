import { Page } from '@playwright/test'

/**
 * LLM-specific navigation helpers
 */

// =====================================================
// Provider Navigation
// =====================================================

export async function goToProvidersPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-providers`)
  // Use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')
}

export async function waitForProvidersPageLoad(page: Page) {
  // Wait for the providers page to load
  // The page shows a provider list in the sidebar which takes time to load from the API
  // Use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')
  // Wait a bit more for the provider list to render
  await page.waitForTimeout(1000)
}

export async function goToProviderDetail(
  page: Page,
  baseURL: string,
  providerId: string
) {
  await page.goto(`${baseURL}/settings/llm-providers/${providerId}`)
  // Use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')
}

export async function clickProviderCard(page: Page, providerName: string) {
  // Wait for provider to be visible in the sidebar menu
  const providerMenuItem = page.locator(`[role="menu"] [role="menuitem"]:has-text("${providerName}")`)
  await providerMenuItem.waitFor({ state: 'visible', timeout: 10000 })

  // Click the provider
  await providerMenuItem.click()

  // Wait for page to load - use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')

  // Wait for provider detail content to render (wait for any card to appear as indicator)
  await page.waitForSelector('.ant-card', { timeout: 10000 })

  // Additional wait for all cards to render, including ones that might be lower on the page
  await page.waitForTimeout(1000)
}

// =====================================================
// Repository Navigation
// =====================================================

export async function goToRepositoriesPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-repositories`)
  // Use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')
}

export async function waitForRepositoriesPageLoad(page: Page) {
  await page.waitForSelector('text=LLM Repositories', { timeout: 30000 })
}

// =====================================================
// Tab Navigation (on Provider Detail page)
// =====================================================

export type ProviderTab = 'models' | 'downloads' | 'settings'

export async function switchToTab(page: Page, tab: ProviderTab) {
  const tabText = tab.charAt(0).toUpperCase() + tab.slice(1)
  await page.click(`text=${tabText}`)
  // Use 'load' instead of 'networkidle' to avoid issues with SSE connections
  await page.waitForLoadState('load')
}

export async function waitForTabLoad(page: Page, tabName: string) {
  await page.waitForSelector(`text=${tabName}`, { timeout: 30000 })
}
