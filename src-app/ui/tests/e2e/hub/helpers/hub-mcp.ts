import { Page, expect } from '@playwright/test'

/**
 * Install MCP server from hub
 */
export async function installMcpServerFromHub(
  page: Page,
  mcpServerId: string,
  customize?: {
    name?: string
    description?: string
  },
) {
  const mcpCard = page.getByTestId(`hub-mcp-card-${mcpServerId}`)
  // Admin sees TWO install buttons ("Install for me" + "Install for the
  // system"); non-admin sees one ("Install"). Both regular flows here go
  // through the user-scope path, so anchor on the "install for me" testid.
  await mcpCard.getByTestId('hub-mcp-install-btn').click()

  // Hub install opens McpServerDrawer prefilled from the manifest so the
  // user can review/edit fields and submit via the normal create endpoint.
  await expect(page.getByTestId('mcp-drawer-form')).toBeVisible({
    timeout: 10000,
  })

  if (customize) {
    if (customize.name) {
      await page.getByTestId('mcp-drawer-name-input').fill(customize.name)
    }
    if (customize.description) {
      await page
        .getByTestId('mcp-drawer-description-textarea')
        .fill(customize.description)
    }
  }

  // The drawer's main submit button (labeled "Create" in create modes).
  await page.getByTestId('mcp-drawer-submit-btn').click()

  // Wait for the success OR auto-disabled warning toast (the connection
  // health probe downgrades to warning when the URL is unreachable; either
  // outcome means the create round-tripped).
  await expect(
    page
      .locator(
        '[data-sonner-toast][data-type="success"], [data-sonner-toast][data-type="warning"]',
      )
      .first(),
  ).toBeVisible({ timeout: 10000 })
}

/**
 * Get MCP card status badge text (or null when absent).
 */
export async function getMcpCardStatus(
  page: Page,
  mcpServerId: string,
): Promise<string | null> {
  const badge = page.getByTestId(`hub-mcp-installed-tag-${mcpServerId}`)

  const visible = await badge.isVisible({ timeout: 10000 }).catch(() => false)
  if (visible) {
    return await badge.textContent()
  }

  return null
}

/**
 * Check if MCP server has been installed (View button visible)
 */
export async function isMcpServerInstalled(
  page: Page,
  mcpServerId: string,
): Promise<boolean> {
  const viewButton = page
    .getByTestId(`hub-mcp-card-${mcpServerId}`)
    .getByTestId('hub-mcp-view-btn')
  return await viewButton.isVisible({ timeout: 10000 }).catch(() => false)
}

/**
 * Get all MCP server cards
 */
export async function getMcpServerCards(page: Page) {
  return page.getByTestId(/^hub-mcp-card-/)
}
