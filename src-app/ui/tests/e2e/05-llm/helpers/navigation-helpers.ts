import { Page } from '@playwright/test'
import { byTestId } from '../../testid'

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
  // Wait for the providers page to load. The provider nav (sidebar of
  // provider buttons + the "Add Provider" button) renders once the
  // provider list resolves; the add-provider nav button is always
  // present when the user can create.
  await page.waitForLoadState('load')
  await page
    .locator('[data-testid^="llm-provider-nav-"]')
    .first()
    .waitFor({ state: 'visible', timeout: 30000 })
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
  // Provider nav buttons render as `llm-provider-nav-${providerId}` and
  // carry the provider name as text. Filter the nav-button collection by
  // the (dynamic) provider name the test created.
  const providerNav = page
    .locator('[data-testid^="llm-provider-nav-"]')
    .filter({ hasText: providerName })
    .first()
  await providerNav.waitFor({ state: 'visible', timeout: 10000 })
  await providerNav.click()

  // Wait for page to load - use 'load' instead of 'networkidle'
  await page.waitForLoadState('load')

  // Provider detail surfaces the Models card once loaded.
  await byTestId(page, 'llm-models-section-card').waitFor({
    state: 'visible',
    timeout: 10000,
  })
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
  await byTestId(page, 'llmrepo-card').waitFor({ state: 'visible', timeout: 30000 })
}

// =====================================================
// Tab Navigation (on Provider Detail page)
// =====================================================

export type ProviderTab = 'models' | 'downloads' | 'settings'

const TAB_CARD: Record<ProviderTab, string> = {
  models: 'llm-models-section-card',
  downloads: 'llm-downloads-section-card',
  settings: 'llm-provider-settings-empty',
}

export async function switchToTab(page: Page, tab: ProviderTab) {
  // The provider detail page renders all sections on one page (no real
  // tab bar); wait for the section's card to be present.
  await byTestId(page, TAB_CARD[tab])
    .first()
    .waitFor({ state: 'visible', timeout: 30000 })
    .catch(() => {})
  await page.waitForLoadState('load')
}

export async function waitForTabLoad(page: Page, tabTestId: string) {
  await byTestId(page, tabTestId).first().waitFor({ state: 'visible', timeout: 30000 })
}
