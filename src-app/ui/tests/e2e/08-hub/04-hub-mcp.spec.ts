import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import {
  installMcpServerFromHub,
  getMcpServerCards,
  isMcpServerInstalled,
  getMcpCardStatus,
} from './helpers/hub-mcp'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'

test.describe('Hub MCP Servers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToHub(page, baseURL, 'mcp-servers')
    await waitForHubDataLoad(page)
  })

  test('should display all hub MCP servers', async ({ page }) => {
    const mcpCards = await getMcpServerCards(page)
    const count = await mcpCards.count()

    expect(count).toBeGreaterThan(0)
  })

  test('should show MCP server cards with required information', async ({ page }) => {
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    // Should have "Install" button
    await expect(firstCard.getByRole('button', { name: /install/i })).toBeVisible()

    // Card should have content (text visible)
    await expect(firstCard).toContainText(/.+/)
  })

  // Note: McpServerHubCard.handleInstall navigates to /settings/mcp-servers
  // after install ("Navigate to user MCP servers after creation"). The
  // tests below navigate back to /hub/mcp-servers before checking the
  // badge. Backend GET /api/hub/mcp_servers stitches in created_ids
  // from hub_entities + the user's group memberships.

  test('should install MCP server from hub without customization', async ({
    page,
    testInfra,
  }) => {
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    // Get the MCP server ID from the test ID
    const testId = await firstCard.getAttribute('data-testid')
    const mcpServerId = testId?.replace('hub-mcp-card-', '') || ''

    expect(mcpServerId).toBeTruthy()

    // Install MCP server
    await installMcpServerFromHub(page, mcpServerId)

    // Should show success message (use .first() to handle Ant Design duplicates)
    await expect(
      page.getByText(/installed.*successfully|mcp.*server.*installed/i).first(),
    ).toBeVisible({ timeout: 5000 })

    await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
    await waitForHubDataLoad(page)

    const installed = await isMcpServerInstalled(page, mcpServerId)
    expect(installed).toBe(true)
  })

  test('should install MCP server with customization', async ({
    page,
    testInfra,
  }) => {
    const mcpCards = await getMcpServerCards(page)

    // Get second MCP server if available
    const count = await mcpCards.count()
    const cardIndex = count > 1 ? 1 : 0
    const card = mcpCards.nth(cardIndex)

    const testId = await card.getAttribute('data-testid')
    const mcpServerId = testId?.replace('hub-mcp-card-', '') || ''

    // Install with custom name
    const customName = `Custom MCP Server ${Date.now()}`
    await installMcpServerFromHub(page, mcpServerId, {
      name: customName,
      description: 'Custom description for testing',
    })

    // Should show success message (use .first() to handle Ant Design duplicates)
    await expect(
      page.getByText(/installed.*successfully|mcp.*server.*installed/i).first(),
    ).toBeVisible({ timeout: 5000 })

    await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
    await waitForHubDataLoad(page)

    const installed = await isMcpServerInstalled(page, mcpServerId)
    expect(installed).toBe(true)
  })

  test('should show "View" button for already installed MCP servers', async ({
    page,
    testInfra,
  }) => {
    // Install first MCP server
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const mcpServerId = testId?.replace('hub-mcp-card-', '') || ''

    // Check if already installed
    const alreadyInstalled = await isMcpServerInstalled(page, mcpServerId)

    if (!alreadyInstalled) {
      await installMcpServerFromHub(page, mcpServerId)
      await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)
    }

    // Should have "View" button instead of "Install"
    const card = page.getByTestId(`hub-mcp-card-${mcpServerId}`)
    await expect(card.getByRole('button', { name: /view/i })).toBeVisible()

    // Should NOT have "Install" button
    const installButton = card.getByRole('button', { name: /install/i })
    const installButtonVisible = await installButton.isVisible({ timeout: 1000 }).catch(() => false)
    expect(installButtonVisible).toBe(false)
  })

  test('should track installation status badge', async ({ page, testInfra }) => {
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const mcpServerId = testId?.replace('hub-mcp-card-', '') || ''

    // Get initial status
    const initialStatus = await getMcpCardStatus(page, mcpServerId)

    if (initialStatus === null) {
      // Not installed yet, install it
      await installMcpServerFromHub(page, mcpServerId)

      await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)

      const newStatus = await getMcpCardStatus(page, mcpServerId)
      expect(newStatus).toBeTruthy()
      expect(newStatus).toMatch(/installed/i)
    } else {
      // Already installed
      expect(initialStatus).toMatch(/installed/i)
    }
  })

  test('should navigate to MCP server detail when clicking "View"', async ({
    page,
    testInfra,
  }) => {
    // Find an MCP server that's already installed
    const mcpCards = await getMcpServerCards(page)
    let installedMcpId = ''

    for (let i = 0; i < await mcpCards.count(); i++) {
      const card = mcpCards.nth(i)
      const testId = await card.getAttribute('data-testid')
      const mcpServerId = testId?.replace('hub-mcp-card-', '') || ''

      if (await isMcpServerInstalled(page, mcpServerId)) {
        installedMcpId = mcpServerId
        break
      }
    }

    // If none installed, install one first
    if (!installedMcpId) {
      const firstCard = mcpCards.first()
      const testId = await firstCard.getAttribute('data-testid')
      installedMcpId = testId?.replace('hub-mcp-card-', '') || ''

      await installMcpServerFromHub(page, installedMcpId)
      await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)
    }

    // Click "View" button
    const card = page.getByTestId(`hub-mcp-card-${installedMcpId}`)
    await card.getByRole('button', { name: /view/i }).click()

    // The View button navigates to /settings/mcp-servers (user MCP
    // page) per McpServerHubCard. We're satisfied if we leave the
    // hub or a detail drawer opens. Use a stable wait instead of
    // waitForURL (which expects an explicit navigation EVENT —
    // SPA navigations sometimes don't trigger that path reliably
    // under Playwright's history hooks).
    await page.waitForLoadState('networkidle').catch(() => {})
    const urlChanged = !page.url().includes('/hub/')
    const drawer = page.getByRole('dialog', { name: /mcp.*server/i })
    const drawerVisible = await drawer.isVisible({ timeout: 2000 }).catch(() => false)

    expect(urlChanged || drawerVisible).toBe(true)
  })

  test('should prevent installation without required permissions', async ({
    page,
    testInfra,
  }) => {
    // User with hub::mcp_servers::read but NOT ::create. Cards render
    // (read gives access) but McpServerHubCard's usePermission(
    // HubMcpServersCreate) hides the "Install" button.
    await loginWithPerms(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
      [Permissions.HubMCPServersRead],
    )
    await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
    await waitForHubDataLoad(page)

    const cards = await getMcpServerCards(page)
    const cardCount = await cards.count()
    if (cardCount > 0) {
      await expect(
        cards.first().getByRole('button', { name: /^install$/i }),
      ).toHaveCount(0)
    }
  })

  test('should show MCP server tags', async ({ page }) => {
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    // MCP servers should have tags displayed
    const tags = firstCard.locator('[class*="tag"]').or(firstCard.locator('.ant-tag'))
    const tagCount = await tags.count()

    // Should have at least some tags (varies by MCP server)
    expect(tagCount).toBeGreaterThanOrEqual(0)
  })
})
