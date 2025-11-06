import { Page } from '@playwright/test'

/**
 * MCP-specific navigation helpers
 */

export async function goToMcpServersPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-servers`)
  await page.waitForLoadState('networkidle')
}

export async function goToMcpAdminPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-admin`)
  await page.waitForLoadState('networkidle')
}

export async function waitForMcpPageLoad(page: Page) {
  // Wait for the heading to be visible (more specific than text=MCP Servers)
  await page.waitForSelector('h4:has-text("MCP Servers")', { timeout: 30000 })
  // Wait for loading spinner to disappear (if it appears)
  await page.waitForSelector('text=Loading MCP servers...', { state: 'hidden', timeout: 5000 }).catch(() => {
    // Loading might be too fast to see, that's ok
  })
  // Wait for content to be ready - either servers or empty state
  await page.waitForSelector('button:has-text("Add Server")', { timeout: 10000 })
}

export async function waitForMcpAdminPageLoad(page: Page) {
  // Wait for the heading to be visible (more specific)
  await page.waitForSelector('h4:has-text("System MCP Servers")', { timeout: 30000 })
  // Wait for loading to complete
  await page.waitForSelector('text=Loading system servers...', { state: 'hidden', timeout: 5000 }).catch(() => {
    // Loading might be too fast to see, that's ok
  })
  // Wait for content to be ready (same button text as user page)
  await page.waitForSelector('button:has-text("Add Server")', { timeout: 10000 })
}
