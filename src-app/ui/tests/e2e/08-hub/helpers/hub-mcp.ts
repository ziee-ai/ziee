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
  await mcpCard.getByRole('button', { name: /install/i }).click()

  // Handle customization modal if it appears
  if (customize) {
    const modal = page.getByRole('dialog', { name: /customize|install/i })
    const modalVisible = await modal.isVisible({ timeout: 2000 }).catch(() => false)

    if (modalVisible) {
      if (customize.name) {
        await modal.getByLabel(/name/i).fill(customize.name)
      }
      if (customize.description) {
        await modal.getByLabel(/description/i).fill(customize.description)
      }
      await modal.getByRole('button', { name: /install|create/i }).click()
    }
  }

  // Wait for success message or navigation
  await expect(
    page.getByRole('alert').or(page.getByText(/installed.*successfully/i)),
  ).toBeVisible({ timeout: 5000 })
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

  const visible = await badge.isVisible({ timeout: 1000 }).catch(() => false)
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
  return await viewButton.isVisible({ timeout: 1000 }).catch(() => false)
}

/**
 * Get all MCP server cards
 */
export async function getMcpServerCards(page: Page) {
  return page.getByTestId(/^hub-mcp-card-/)
}
