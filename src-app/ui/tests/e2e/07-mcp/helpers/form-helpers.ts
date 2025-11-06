import { Page, expect } from '@playwright/test'

/**
 * MCP-specific form helpers
 */

export interface McpServerFormData {
  name: string
  displayName: string
  description?: string
  transportType: 'stdio' | 'http' | 'sse'
  // stdio specific
  command?: string
  args?: string[] // JSON array
  env?: Record<string, string> // JSON object
  // http/sse specific
  url?: string
  enabled?: boolean
}

export async function openAddServerDrawer(page: Page, isSystemServer = false) {
  await page.click('button:has-text("Add Server")')
  // Wait for specific drawer title, not generic .ant-drawer-open class
  const drawerTitle = isSystemServer ? 'Add System Server' : 'Add MCP Server'
  await page.waitForSelector(`.ant-drawer-title:has-text("${drawerTitle}")`, { timeout: 5000 })
}

export async function fillMcpServerForm(page: Page, data: McpServerFormData) {
  // Name field (only visible in create mode)
  const nameField = page.locator('input#name')
  if (await nameField.isVisible()) {
    await nameField.fill(data.name)
  }

  // Display name
  await page.fill('input#display_name', data.displayName)

  // Description (optional)
  if (data.description) {
    await page.fill('textarea#description', data.description)
  }

  // Transport type
  await page.click('.ant-select:has(input#transport_type)')
  await page.click(`.ant-select-item-option:has-text("${data.transportType === 'stdio' ? 'Standard I/O' : data.transportType === 'http' ? 'HTTP' : 'Server-Sent Events'}")`)

  // Wait for transport-specific fields to appear
  await page.waitForTimeout(500)

  if (data.transportType === 'stdio') {
    // Command
    if (data.command) {
      await page.fill('input#command', data.command)
    }

    // Arguments (JSON array)
    if (data.args) {
      await page.fill('textarea#args', JSON.stringify(data.args))
    }

    // Environment variables (JSON object)
    if (data.env) {
      await page.fill('textarea#env', JSON.stringify(data.env, null, 2))
    }
  } else {
    // URL for http/sse
    if (data.url) {
      await page.fill('input#url', data.url)
    }
  }

  // Enabled switch
  if (data.enabled !== undefined) {
    const switchButton = page.locator('.ant-switch').last()
    const isChecked = await switchButton.evaluate((el) =>
      el.classList.contains('ant-switch-checked')
    )
    if (isChecked !== data.enabled) {
      await switchButton.click()
    }
  }
}

export async function submitMcpServerForm(page: Page, action: 'create' | 'update' = 'create') {
  const buttonText = action === 'create' ? 'Create Server' : 'Update Server'
  const drawerTitle = action === 'create' ? 'Add MCP Server' : 'Edit MCP Server'

  await page.click(`button:has-text("${buttonText}")`)

  // Wait for specific drawer to close by waiting for its title to disappear
  await page.waitForSelector(`.ant-drawer-title:has-text("${drawerTitle}")`, {
    state: 'hidden',
    timeout: 10000
  })
}

export async function clickEditServerButton(page: Page, serverName: string) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`)
  await serverCard.locator('button:has-text("Edit")').click()
  // Wait for specific Edit drawer title, not generic .ant-drawer-open class
  await page.waitForSelector('.ant-drawer-title:has-text("Edit MCP Server")', { timeout: 5000 })
}

export async function deleteServer(page: Page, serverName: string) {
  // This function would be for delete functionality if/when implemented
  // For now, just a placeholder
}

export async function verifyServerExists(page: Page, serverName: string) {
  await expect(page.locator(`.ant-card:has-text("${serverName}")`)).toBeVisible()
}

export async function verifyServerNotExists(page: Page, serverName: string) {
  await expect(page.locator(`.ant-card:has-text("${serverName}")`)).not.toBeVisible()
}

export async function toggleServerEnabled(page: Page, serverName: string) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`)
  const switchButton = serverCard.locator('.ant-switch').first()
  await switchButton.click()
  await page.waitForTimeout(1000) // Wait for update to complete
}

export async function verifyServerEnabled(page: Page, serverName: string, enabled: boolean) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`)
  const switchButton = serverCard.locator('.ant-switch').first()
  const isChecked = await switchButton.evaluate((el) =>
    el.classList.contains('ant-switch-checked')
  )
  expect(isChecked).toBe(enabled)
}
