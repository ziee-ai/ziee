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
  // Admin sees TWO install buttons ("Install for me" + "Install for
  // the system"); non-admin sees one ("Install"). Both regular flows
  // we exercise here go through the user-scope path, so anchor on the
  // testid for "install for me" specifically. The /install/i name
  // matcher would hit a strict-mode violation when run as admin.
  await mcpCard.getByTestId('hub-mcp-install-btn').click()

  // Hub install no longer materializes the server silently — it now
  // opens McpServerDrawer prefilled from the manifest so the user can
  // review/edit fields (transports/env/headers gated by policy) and
  // submit via the normal create endpoint with hub_id forwarded.
  // Anchor on the drawer title (visibility is on the wrapper, not
  // `.ant-drawer-content`, which is in the DOM even when closed).
  await expect(
    page.locator('.ant-drawer-title:has-text("MCP Server")').first(),
  ).toBeVisible({ timeout: 10000 })

  if (customize) {
    if (customize.name) {
      await page.getByLabel('Name', { exact: true }).fill(customize.name)
    }
    if (customize.description) {
      await page.getByLabel('Description').fill(customize.description)
    }
  }

  // The drawer's main submit button is labeled "Create" for both
  // create + create-system modes. Use the open-state drawer + a
  // text-based locator (getByRole strict mode matches by accessible
  // name which can include surrounding whitespace).
  await page
    .locator('.ant-drawer-open .ant-btn-primary:has-text("Create")')
    .first()
    .click()

  // Wait for the success or auto-disabled warning toast (the new
  // connection-health probe downgrades to warning when the URL is
  // unreachable; either outcome means the create round-tripped).
  await expect(
    page.locator('.ant-message-success, .ant-message-warning').first(),
  ).toBeVisible({ timeout: 10000 })
}

/**
 * Get MCP card status badge
 */
export async function getMcpCardStatus(
  page: Page,
  mcpServerId: string,
): Promise<string | null> {
  const mcpCard = page.getByTestId(`hub-mcp-card-${mcpServerId}`)
  const badge = mcpCard.getByText(/installed/i)

  // Allow several seconds — see hub-assistants helper for rationale.
  const visible = await badge.isVisible({ timeout: 10000 }).catch(() => false)
  if (visible) {
    return await badge.textContent()
  }

  return null
}

/**
 * Check if MCP server has "View" button (indicating it's been installed)
 */
export async function isMcpServerInstalled(
  page: Page,
  mcpServerId: string,
): Promise<boolean> {
  const mcpCard = page.getByTestId(`hub-mcp-card-${mcpServerId}`)
  const viewButton = mcpCard.getByRole('button', { name: /view/i })
  return await viewButton.isVisible({ timeout: 10000 }).catch(() => false)
}

/**
 * Get all MCP server cards
 */
export async function getMcpServerCards(page: Page) {
  return page.getByTestId(/^hub-mcp-card-/)
}
