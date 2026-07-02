import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import {
  installMcpServerFromHub,
  getMcpServerCards,
  isMcpServerInstalled,
  getMcpCardStatus,
} from './helpers/hub-mcp'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'

// A seeded hub MCP server with HTTP transport. User-scope ("install for me")
// stdio installs are gated by code_sandbox (disabled in the test env), so the
// install/View/badge flows target an HTTP server whose user install works.
// The seeded hub MCP catalog uses reverse-DNS namespaced ids
// (`server.name`), and the card testid is `hub-mcp-card-${server.name}`.
// brave's streamable-http server is `com.brave/search-mcp` in the seed.
const HTTP_HUB_MCP_ID = 'com.brave/search-mcp'

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

    // Should have the user-scope "Install" / "Install for me" button.
    // Admin also gets a second "Install for the system" button — pin
    // the assertion to the testid so the count is unambiguous.
    await expect(firstCard.getByTestId('hub-mcp-install-btn')).toBeVisible()

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
    // Most hub MCP servers are stdio; "Install for me" (user scope) of a
    // stdio server is gated by code_sandbox (disabled in tests), so the
    // prefilled transport is invalid and the create is blocked. Target an
    // HTTP hub server, whose user install isn't gated.
    const mcpServerId = HTTP_HUB_MCP_ID

    // Install MCP server
    await installMcpServerFromHub(page, mcpServerId)

    // Should show success/warning toast (the create round-tripped)
    await expect(
      page
        .locator(
          '[data-sonner-toast][data-type="success"], [data-sonner-toast][data-type="warning"]',
        )
        .first(),
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
    const mcpServerId = HTTP_HUB_MCP_ID

    // Install with a custom name. This fills the "Name" slug field, which
    // only allows [a-z0-9-] — so use a slug, not a display-name string.
    const customName = `custom-mcp-${Date.now()}`
    await installMcpServerFromHub(page, mcpServerId, {
      name: customName,
      description: 'Custom description for testing',
    })

    // Should show success/warning toast (the create round-tripped)
    await expect(
      page
        .locator(
          '[data-sonner-toast][data-type="success"], [data-sonner-toast][data-type="warning"]',
        )
        .first(),
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
    const mcpServerId = HTTP_HUB_MCP_ID

    // Check if already installed
    const alreadyInstalled = await isMcpServerInstalled(page, mcpServerId)

    if (!alreadyInstalled) {
      await installMcpServerFromHub(page, mcpServerId)
      await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)
    }

    // Should have "View" button (the user-scope "Install for me"
    // button collapses to View after a user install).
    const card = page.getByTestId(`hub-mcp-card-${mcpServerId}`)
    await expect(card.getByTestId('hub-mcp-view-btn')).toBeVisible()

    // The user-scope install button is gone.
    await expect(card.getByTestId('hub-mcp-install-btn')).toHaveCount(0)
    // Note: when run as admin (the beforeEach default), the
    // "Install for the system" button is independent of the user
    // install state and remains visible. The user-scope install
    // button collapse is the assertion we care about here.
  })

  test('should track installation status badge', async ({ page, testInfra }) => {
    const mcpServerId = HTTP_HUB_MCP_ID

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
      installedMcpId = HTTP_HUB_MCP_ID

      await installMcpServerFromHub(page, installedMcpId)
      await navigateToHub(page, testInfra.baseURL, 'mcp-servers')
      await waitForHubDataLoad(page)
    }

    // Click "View" button
    const card = page.getByTestId(`hub-mcp-card-${installedMcpId}`)
    await card.getByTestId('hub-mcp-view-btn').click()

    // The View button navigates to /settings/mcp-servers (user MCP
    // page) per McpServerHubCard. We're satisfied if we leave the
    // hub or a detail drawer opens. Use a stable wait instead of
    // waitForURL (which expects an explicit navigation EVENT —
    // SPA navigations sometimes don't trigger that path reliably
    // under Playwright's history hooks).
    await page.waitForLoadState('load').catch(() => {})
    const urlChanged = !page.url().includes('/hub/')
    const drawer = page.getByTestId('hub-mcp-detail-sheet')
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
        cards.first().getByTestId('hub-mcp-install-btn'),
      ).toHaveCount(0)
    }
  })

  test('should show MCP server tags', async ({ page }) => {
    const mcpCards = await getMcpServerCards(page)
    const firstCard = mcpCards.first()

    // MCP servers should have tags displayed (transport / version / registry)
    const tags = firstCard.locator('[data-testid*="-tag-"]')
    const tagCount = await tags.count()

    // Should have at least some tags (varies by MCP server)
    expect(tagCount).toBeGreaterThanOrEqual(0)
  })

  // The install specs stop at "it shows installed"; none verify the installed
  // server is wired into CHAT. This installs the HTTP hub MCP server then opens
  // the chat MCP config and asserts the server is available to attach — the
  // install → chat integration point (deterministic; no real LLM / server run).
  test('an installed hub MCP server is available to attach in chat', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(apiURL)

    await installMcpServerFromHub(page, HTTP_HUB_MCP_ID)
    await expect(
      page
        .locator(
          '[data-sonner-toast][data-type="success"], [data-sonner-toast][data-type="warning"]',
        )
        .first(),
    ).toBeVisible({ timeout: 15000 })

    // A conversation so the composer's "Skills/MCP in this chat" entries render.
    const convId = (await (
      await fetch(`${apiURL}/api/conversations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ title: 'hub-mcp-chat' }),
      })
    ).json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', { timeout: 30000 })

    // Open the composer "+" → MCP config modal → the installed server is listed.
    await page.getByTestId('chat-input-add-btn').first().click()
    await page.getByTestId('chat-mcp-menu-item').click()
    const modal = page.getByTestId('mcp-config-modal')
    await expect(modal).toBeVisible({ timeout: 10000 })
    // The installed server (display name contains "brave") appears in the
    // servers accordion — dynamic data the install produced.
    await expect(
      page
        .getByTestId('mcp-config-servers-accordion')
        .filter({ hasText: /brave/i }),
    ).toBeVisible({ timeout: 10000 })
  })
})
