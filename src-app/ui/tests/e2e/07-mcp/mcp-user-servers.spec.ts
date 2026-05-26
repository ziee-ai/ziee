import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from './helpers/navigation-helpers'
import {
  openAddServerDrawer,
  fillMcpServerForm,
  submitMcpServerForm,
  verifyServerExists,
  clickEditServerButton,
  toggleServerEnabled,
  verifyServerEnabled,
  type McpServerFormData,
} from './helpers/form-helpers'

test.describe('MCP - User Servers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)
  })

  test('should pass accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('should display MCP Servers page', async ({ page }) => {
    // Use more specific locators to avoid strict mode violations
    await expect(page.getByRole('heading', { name: 'MCP Servers' })).toBeVisible()
    await expect(page.locator('text=Manage Model Context Protocol servers')).toBeVisible()
    await expect(page.locator('button:has-text("Add Server")')).toBeVisible()
  })

  test('should display search and filter controls', async ({ page }) => {
    await expect(page.locator('input[placeholder="Search servers..."]')).toBeVisible()
    await expect(page.locator('.ant-select:has-text("All Servers")')).toBeVisible()
  })

  test('should display system servers from default group', async ({ page }) => {
    // Default servers from migration should be visible
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).toBeVisible()
    await expect(page.locator('.ant-card:has(.ant-tag:has-text("System"))')).toBeVisible()
  })

  test('should open Add Server drawer', async ({ page }) => {
    await openAddServerDrawer(page)
    await expect(page.locator('.ant-drawer-title:has-text("Add MCP Server")')).toBeVisible()
    await expect(page.getByLabel('Name', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Display Name')).toBeVisible()
  })

  test('should create HTTP MCP server successfully', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-http-server',
      displayName: 'Test HTTP Server',
      description: 'A test HTTP MCP server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create')

    // Verify success message
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  test('should create stdio MCP server with arguments', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-stdio-server',
      displayName: 'Test Stdio Server',
      description: 'A test stdio MCP server',
      transportType: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-test'],
      env: { TEST_VAR: 'test_value' },
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create')

    // Verify success message
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  test('should validate required fields', async ({ page }) => {
    await openAddServerDrawer(page)

    // Try to submit without filling required fields
    await page.locator('.ant-drawer.ant-drawer-open').last().locator('.ant-btn-primary').click()

    // Verify validation errors - check for at least one error (there will be multiple)
    await expect(page.locator('.ant-form-item-explain-error').first()).toBeVisible()
  })

  test('should validate JSON format for arguments', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-invalid-args',
      displayName: 'Test Invalid Args',
      transportType: 'stdio',
      command: 'npx',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)

    // Fill args with invalid JSON
    await page.getByLabel('Arguments').fill('not valid json')

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('.ant-btn-primary').click()

    // Verify error message
    await expect(page.locator('.ant-message-error:has-text("Invalid JSON")')).toBeVisible({ timeout: 5000 })
  })

  test('should validate JSON object for environment variables', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-invalid-env',
      displayName: 'Test Invalid Env',
      transportType: 'stdio',
      command: 'npx',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)

    // Fill env with JSON array instead of object
    await page.getByLabel('Environment Variables').fill('["not", "an", "object"]')

    await page.locator('.ant-drawer.ant-drawer-open').last().locator('.ant-btn-primary').click()

    // Verify error message
    await expect(page.locator('.ant-message-error:has-text("must be a JSON object")')).toBeVisible({ timeout: 5000 })
  })

  test('should edit existing server', async ({ page }) => {
    // First create a server
    const serverData: McpServerFormData = {
      name: 'test-edit-server',
      displayName: 'Test Edit Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create')

    await page.waitForTimeout(1000)

    // Edit the server
    await clickEditServerButton(page, serverData.displayName)

    // Verify drawer opens with pre-filled data
    await expect(page.locator('.ant-drawer-title:has-text("Edit MCP Server")')).toBeVisible()
    await expect(page.getByLabel('Display Name')).toHaveValue(serverData.displayName)

    // Update display name
    const newDisplayName = 'Updated Test Server'
    await page.getByLabel('Display Name').fill(newDisplayName)

    await submitMcpServerForm(page, 'update')

    // Verify success message - check for specific "updated" text
    await expect(page.locator('.ant-message-success:has-text("updated")')).toBeVisible({ timeout: 5000 })

    // Verify updated name appears
    await verifyServerExists(page, newDisplayName)
  })

  test('should toggle server enabled state', async ({ page }) => {
    // First create a server
    const serverData: McpServerFormData = {
      name: 'test-toggle-server',
      displayName: 'Test Toggle Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create')

    await page.waitForTimeout(1000)

    // Toggle enabled state
    await toggleServerEnabled(page, serverData.displayName)

    // Verify success message - check for specific "disabled" text
    await expect(page.locator('.ant-message-success:has-text("disabled")')).toBeVisible({ timeout: 5000 })

    // Verify switch state changed
    await verifyServerEnabled(page, serverData.displayName, false)
  })

  test('should filter servers by search term', async ({ page }) => {
    const searchInput = page.locator('input[placeholder="Search servers..."]')
    await searchInput.fill('Web Fetch')

    // Should show Web Fetch server
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).toBeVisible()

    // Should not show Filesystem server
    await expect(page.locator('.ant-card:has-text("Filesystem")')).not.toBeVisible()
  })

  test('should filter servers by status', async ({ page }) => {
    // Click status filter
    await page.click('.ant-select:has-text("All Servers")')

    // Select 'System' filter
    await page.click('.ant-select-item-option:has-text("System")')

    // Should show only system servers
    await expect(page.locator('.ant-card:has(.ant-tag:has-text("System"))')).toBeVisible()
  })

  test('should clear all filters', async ({ page }) => {
    // Apply search filter
    await page.fill('input[placeholder="Search servers..."]', 'test')

    // Verify clear button appears
    await expect(page.locator('button:has-text("Clear all")')).toBeVisible()

    // Click clear all
    await page.click('button:has-text("Clear all")')

    // Verify filters are cleared
    await expect(page.locator('input[placeholder="Search servers..."]')).toHaveValue('')
  })

  test('should display system servers as read-only', async ({ page }) => {
    const systemServerCard = page.locator('.ant-card:has-text("Web Fetch")')

    // System servers should have System tag
    await expect(systemServerCard.locator('.ant-tag:has-text("System")')).toBeVisible()

    // System servers should not have Edit button visible
    await expect(systemServerCard.locator('button:has-text("Edit")')).not.toBeVisible()
  })

  test('should display empty state when no servers match filter', async ({ page }) => {
    // Search for non-existent server
    await page.fill('input[placeholder="Search servers..."]', 'nonexistent-server-xyz')

    // Should display empty state
    await expect(page.locator('text=No servers match your search criteria')).toBeVisible()
  })

  // ──────────────────────────────────────────────────────────────────────
  // New coverage added for feat/mcp-rewrite-v2
  // ──────────────────────────────────────────────────────────────────────

  test('should default sort to "Date Added" on user MCP page', async ({ page }) => {
    await expect(page.locator('.ant-select:has-text("Date Added")')).toBeVisible()
    await page.click('.ant-select:has-text("Date Added")')
    await expect(page.locator('.ant-select-item-option:has-text("Date Added")')).toBeVisible()
    await page.keyboard.press('Escape')
  })

  test('should hide Delete on group-assigned system servers (read-only)', async ({ page }) => {
    // Web Fetch ships in the default group; verify it appears with no Delete button.
    const card = page.locator('.ant-card:has-text("Web Fetch")').first()
    await expect(card).toBeVisible()
    await expect(card.locator('[data-testid="mcp-server-delete-btn"]')).not.toBeVisible()
  })
})
