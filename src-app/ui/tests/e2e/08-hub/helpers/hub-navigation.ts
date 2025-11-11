import { Page, expect } from '@playwright/test'

export type HubTab = 'models' | 'assistants' | 'mcp-servers'

/**
 * Navigate to hub page with optional tab
 */
export async function navigateToHub(page: Page, baseURL: string, tab?: HubTab) {
  const targetTab = tab || 'models'
  await page.goto(`${baseURL}/hub/${targetTab}`)
  await expect(page).toHaveURL(new RegExp(`/hub/${targetTab}`))
}

/**
 * Switch between hub tabs - navigates via URL for reliability
 */
export async function switchHubTab(page: Page, tab: HubTab) {
  // Get current base URL
  const currentURL = page.url()
  const baseURL = currentURL.split('/hub')[0]

  // Navigate to the new tab
  await page.goto(`${baseURL}/hub/${tab}`)

  //Wait for URL to update
  await expect(page).toHaveURL(new RegExp(`/hub/${tab}`))
}

/**
 * Wait for hub data to finish loading
 */
export async function waitForHubDataLoad(page: Page) {
  // Wait for either cards to appear or "Loading..." to disappear
  // Hub uses LazyComponentRenderer which shows "Loading..." fallback
  try {
    // Wait for at least one card OR a "no results" message
    await Promise.race([
      page.locator('[data-testid^="hub-"]').first().waitFor({ state: 'visible', timeout: 5000 }),
      page.getByText(/no.*results|no.*found/i).waitFor({ state: 'visible', timeout: 5000 }),
      page.waitForTimeout(2000) // Fallback timeout
    ])
  } catch (error) {
    // If all fail, just wait a bit and continue - page might have loaded already
    console.log('Hub data load timeout, continuing anyway')
  }
}

/**
 * Get currently active tab
 */
export async function getActiveHubTab(page: Page): Promise<HubTab> {
  const url = page.url()
  if (url.includes('/hub/assistants')) return 'assistants'
  if (url.includes('/hub/mcp-servers')) return 'mcp-servers'
  return 'models'
}

/**
 * Refresh hub data by clicking the refresh button
 */
export async function refreshHubData(page: Page) {
  try {
    const refreshButton = page.getByRole('button', { name: /refresh/i })
    const visible = await refreshButton.isVisible({ timeout: 2000 }).catch(() => false)
    if (visible) {
      await refreshButton.click()
      // Wait for refresh to complete
      await page.waitForTimeout(1000)
    }
  } catch (error) {
    // Refresh button not found or not clickable, skip
    console.log('Refresh button not found, skipping refresh')
  }
}
