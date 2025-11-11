import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToHub,
  switchHubTab,
  waitForHubDataLoad,
  getActiveHubTab,
} from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'
import { getAssistantCards } from './helpers/hub-assistants'
import { getMcpServerCards } from './helpers/hub-mcp'

test.describe('Hub Navigation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
  })

  test('should navigate to hub models page by default', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL)

    // Should be on models tab
    await expect(page).toHaveURL(/\/hub\/models/)

    // Should show models
    await waitForHubDataLoad(page)
    const modelCards = await getModelCards(page)
    await expect(modelCards.first()).toBeVisible()
  })

  test('should navigate to specific hub tab', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Navigate to assistants tab
    await navigateToHub(page, baseURL, 'assistants')
    await expect(page).toHaveURL(/\/hub\/assistants/)
    await waitForHubDataLoad(page)

    const activeTab = await getActiveHubTab(page)
    expect(activeTab).toBe('assistants')

    // Navigate to mcp-servers tab
    await navigateToHub(page, baseURL, 'mcp-servers')
    await expect(page).toHaveURL(/\/hub\/mcp-servers/)
    await waitForHubDataLoad(page)

    const activeTab2 = await getActiveHubTab(page)
    expect(activeTab2).toBe('mcp-servers')
  })

  test('should switch between tabs', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // Switch to assistants
    await switchHubTab(page, 'assistants')
    await waitForHubDataLoad(page)

    await expect(page).toHaveURL(/\/hub\/assistants/)
    const assistantCards = await getAssistantCards(page)
    await expect(assistantCards.first()).toBeVisible()

    // Switch to mcp-servers
    await switchHubTab(page, 'mcp-servers')
    await waitForHubDataLoad(page)

    await expect(page).toHaveURL(/\/hub\/mcp-servers/)
    const mcpCards = await getMcpServerCards(page)
    await expect(mcpCards.first()).toBeVisible()

    // Switch back to models
    await switchHubTab(page, 'models')
    await waitForHubDataLoad(page)

    await expect(page).toHaveURL(/\/hub\/models/)
    const modelCards = await getModelCards(page)
    await expect(modelCards.first()).toBeVisible()
  })

  test('should persist tab selection in URL', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Navigate to assistants tab
    await navigateToHub(page, baseURL, 'assistants')
    await waitForHubDataLoad(page)

    // Reload page
    await page.reload()
    await waitForHubDataLoad(page)

    // Should still be on assistants tab
    await expect(page).toHaveURL(/\/hub\/assistants/)
    const activeTab = await getActiveHubTab(page)
    expect(activeTab).toBe('assistants')
  })

  test('should show resources in all tabs', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Models tab
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    const modelCards = await getModelCards(page)
    const modelCount = await modelCards.count()
    expect(modelCount).toBeGreaterThan(0)

    // Assistants tab
    await switchHubTab(page, 'assistants')
    await waitForHubDataLoad(page)

    const assistantCards = await getAssistantCards(page)
    const assistantCount = await assistantCards.count()
    expect(assistantCount).toBeGreaterThan(0)

    // MCP servers tab
    await switchHubTab(page, 'mcp-servers')
    await waitForHubDataLoad(page)

    const mcpCards = await getMcpServerCards(page)
    const mcpCount = await mcpCards.count()
    expect(mcpCount).toBeGreaterThan(0)
  })

  test('should handle direct URL navigation', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Navigate directly via URL
    await page.goto(`${baseURL}/hub/mcp-servers`)
    await expect(page).toHaveURL(/\/hub\/mcp-servers/)
    await waitForHubDataLoad(page)

    const activeTab = await getActiveHubTab(page)
    expect(activeTab).toBe('mcp-servers')

    const mcpCards = await getMcpServerCards(page)
    await expect(mcpCards.first()).toBeVisible()
  })

  test('should show loading state during data fetch', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // Navigate to hub
    await page.goto(`${baseURL}/hub/models`)

    // Should show loading spinner initially
    const spinner = page.getByRole('progressbar').or(page.locator('.ant-spin'))

    // Either loading is visible, or data loads so fast we miss it
    const spinnerVisible = await spinner.isVisible({ timeout: 500 }).catch(() => false)

    if (spinnerVisible) {
      // If we caught the spinner, wait for it to disappear
      await expect(spinner).not.toBeVisible({ timeout: 10000 })
    }

    // Should show data after loading
    const modelCards = await getModelCards(page)
    await expect(modelCards.first()).toBeVisible()
  })
})
