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
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  // User-scope stdio creates are gated by `code_sandbox.enabled` per
  // the MCP user policy (migration 84). The test infra runs with
  // sandbox disabled, so the policy load filters `'stdio'` out of
  // `allowed_transports` and the transport dropdown / submit endpoint
  // both refuse stdio for non-admin users. The user-stdio happy path
  // is covered at the integration tier — see
  // `server/tests/mcp/run_in_sandbox_test.rs::user_mode_stdio_create_is_gated_by_user_policy`
  // and the dedicated `user_create_stdio_is_gated_by_sandbox_policy_not_host_allowlist`
  // case for the gate reason. The admin/system stdio happy path lives
  // in `mcp-admin-servers.spec.ts::should create system stdio server
  // successfully` (system creates bypass the user policy).
  test.skip('should create stdio MCP server with arguments', async () => {})

  test('should validate required fields', async ({ page }) => {
    await openAddServerDrawer(page)

    // Try to submit without filling required fields
    await page.locator('.ant-drawer.ant-drawer-open').last().locator('.ant-btn-primary').click()

    // Verify validation errors - check for at least one error (there will be multiple)
    await expect(page.locator('.ant-form-item-explain-error').first()).toBeVisible()
  })

  // The Arguments / Environment-Variables fields only exist for STDIO
  // transport, but user-scope stdio is gated by `code_sandbox.enabled`
  // (disabled in the test env), so the policy filters 'stdio' out of a
  // non-admin user's allowed transports and these fields are unreachable
  // here (same gating that skips the user-stdio create above). The
  // Arguments JSON-validation path is exercised against an admin/system
  // stdio server in `mcp-admin-servers.spec.ts`; the old env-var
  // "must be a JSON object" check no longer exists — env vars are now a
  // structured key/value editor (KeyValueSecretEditor), not a JSON field.
  test.skip('should validate JSON format for arguments', async () => {})

  test.skip('should validate JSON object for environment variables', async () => {})

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

  // Sort dropdown removed from the user MCP page by bcc2047 —
  // backend orders the list (`is_system ASC, display_name ASC`).
  test.skip('should default sort to "Date Added" on user MCP page', async () => {})

  test('should hide Delete on group-assigned system servers (read-only)', async ({ page }) => {
    // Web Fetch ships in the default group; verify it appears with no Delete button.
    const card = page.locator('.ant-card:has-text("Web Fetch")').first()
    await expect(card).toBeVisible()
    await expect(card.locator('[data-testid="mcp-server-delete-btn"]')).not.toBeVisible()
  })
})
