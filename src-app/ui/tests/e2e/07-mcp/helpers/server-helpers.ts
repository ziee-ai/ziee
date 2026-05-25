import { Page } from '@playwright/test'
import { fillMcpServerForm, submitMcpServerForm, type McpServerFormData } from './form-helpers'
import { goToMcpAdminPage, waitForMcpAdminPageLoad } from './navigation-helpers'

/**
 * MCP Server CRUD helpers (for system servers)
 */

export async function createSystemServer(
  page: Page,
  baseURL: string,
  name: string,
  displayName: string,
  description?: string
): Promise<void> {
  const serverData: McpServerFormData = {
    name,
    displayName,
    description,
    transportType: 'stdio',
    command: 'node',
    args: ['server.js'],
    enabled: true,
  }

  await goToMcpAdminPage(page, baseURL)
  await waitForMcpAdminPageLoad(page)

  // Click Add Server button
  await page.click('button:has-text("Add Server")')

  // Wait for drawer
  await page.waitForSelector('.ant-drawer-title:has-text("Add System Server")', { timeout: 5000 })

  // Fill and submit form
  await fillMcpServerForm(page, serverData)
  await submitMcpServerForm(page, 'create', true)

  // Verify success (drawer closes automatically)
  await page.waitForTimeout(1000)
}

export async function deleteSystemServer(
  page: Page,
  serverDisplayName: string
): Promise<void> {
  // Find the server card
  const serverCard = page.locator(`.ant-card:has-text("${serverDisplayName}")`).first()
  await serverCard.waitFor({ state: 'visible', timeout: 10000 })

  // Click delete button (if exists)
  const deleteButton = serverCard.locator('button[aria-label="Delete"]')
  if (await deleteButton.count() > 0) {
    await deleteButton.click()

    // Confirm deletion
    await page.waitForSelector('.ant-popconfirm', { state: 'visible', timeout: 5000 })
    await page.click('.ant-popconfirm .ant-btn-primary')

    // Wait for success message
    await page.waitForSelector('.ant-message-success', { timeout: 10000 })
  }
}
