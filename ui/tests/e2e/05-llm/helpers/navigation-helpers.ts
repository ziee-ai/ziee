import { Page } from '@playwright/test'

/**
 * LLM-specific navigation helpers
 */

// =====================================================
// Provider Navigation
// =====================================================

export async function goToProvidersPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-providers`)
  await page.waitForLoadState('networkidle')
}

export async function waitForProvidersPageLoad(page: Page) {
  // Wait for the providers page to load
  // The page shows a provider list in the sidebar which takes time to load from the API
  await page.waitForLoadState('networkidle')
  // Wait a bit more for the provider list to render
  await page.waitForTimeout(1000)
}

export async function goToProviderDetail(
  page: Page,
  baseURL: string,
  providerId: string
) {
  await page.goto(`${baseURL}/settings/llm-providers/${providerId}`)
  await page.waitForLoadState('networkidle')
}

export async function clickProviderCard(page: Page, providerName: string) {
  await page.click(`text=${providerName}`)
  await page.waitForLoadState('networkidle')
}

// =====================================================
// Repository Navigation
// =====================================================

export async function goToRepositoriesPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-repositories`)
  await page.waitForLoadState('networkidle')
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
  await page.waitForLoadState('networkidle')
}

export async function waitForTabLoad(page: Page, tabName: string) {
  await page.waitForSelector(`text=${tabName}`, { timeout: 30000 })
}
