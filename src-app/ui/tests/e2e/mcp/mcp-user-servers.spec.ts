import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
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
    // The page chrome rendered: Add button + search + status filter are present.
    await expect(byTestId(page, 'mcp-settings-add-btn')).toBeVisible()
    await expect(byTestId(page, 'mcp-settings-search-input')).toBeVisible()
    await expect(byTestId(page, 'mcp-settings-status-select')).toBeVisible()
  })

  test('should display search and filter controls', async ({ page }) => {
    await expect(byTestId(page, 'mcp-settings-search-input')).toBeVisible()
    await expect(byTestId(page, 'mcp-settings-status-select')).toBeVisible()
  })

  test('should display system servers from default group', async ({ page }) => {
    // Default servers from migration should be visible
    await expect(
      page.getByTestId(/^mcp-server-card-/).filter({ hasText: 'Web Fetch' }),
    ).toBeVisible()
    await expect(
      page.getByTestId(/^mcp-server-card-/).filter({ has: page.getByTestId('mcp-server-system-tag') }).first(),
    ).toBeVisible()
  })

  test('should open Add Server drawer', async ({ page }) => {
    await openAddServerDrawer(page)
    await expect(byTestId(page, 'mcp-drawer-form')).toBeVisible()
    await expect(byTestId(page, 'mcp-drawer-name-input')).toBeVisible()
    await expect(byTestId(page, 'mcp-drawer-display-name-input')).toBeVisible()
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

    // Success closes the drawer (submitMcpServerForm waits for that).
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })

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

    // Try to submit without filling required fields → the drawer stays open
    // (validation blocks submit; the form is not detached).
    await byTestId(page, 'mcp-drawer-submit-btn').click()
    await expect(byTestId(page, 'mcp-drawer-form')).toBeVisible()
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
    // Edit mode: the create-only Name field is gone; Display Name is prefilled.
    await expect(byTestId(page, 'mcp-drawer-name-input')).toHaveCount(0)
    await expect(byTestId(page, 'mcp-drawer-display-name-input')).toHaveValue(serverData.displayName)

    // Update display name
    const newDisplayName = 'Updated Test Server'
    await byTestId(page, 'mcp-drawer-display-name-input').fill(newDisplayName)

    await submitMcpServerForm(page, 'update')
    // Success closes the drawer.
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })

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

    // toggleServerEnabled waits for the PUT round-trip; the switch now reflects
    // the disabled state.
    await verifyServerEnabled(page, serverData.displayName, false)
  })

  test('should filter servers by search term', async ({ page }) => {
    await byTestId(page, 'mcp-settings-search-input').fill('Web Fetch')

    // Should show Web Fetch server
    await expect(
      page.getByTestId(/^mcp-server-card-/).filter({ hasText: 'Web Fetch' }),
    ).toBeVisible()

    // Should not show Filesystem server
    await expect(
      page.getByTestId(/^mcp-server-card-/).filter({ hasText: 'Filesystem' }),
    ).toHaveCount(0)
  })

  test('should filter servers by status', async ({ page }) => {
    // Open the status filter and select 'System'.
    await byTestId(page, 'mcp-settings-status-select').click()
    await byTestId(page, 'mcp-settings-status-select-opt-system').click()

    // Should show only system servers (each card carries the System tag).
    await expect(
      page.getByTestId(/^mcp-server-card-/).filter({ has: page.getByTestId('mcp-server-system-tag') }).first(),
    ).toBeVisible()
  })

  test('should clear all filters', async ({ page }) => {
    // Apply search filter
    await byTestId(page, 'mcp-settings-search-input').fill('test')

    // The Clear-all button appears, clears filters when clicked.
    await expect(byTestId(page, 'mcp-settings-clear-filters-btn')).toBeVisible()
    await byTestId(page, 'mcp-settings-clear-filters-btn').click()
    await expect(byTestId(page, 'mcp-settings-search-input')).toHaveValue('')
  })

  test('should display system servers as read-only', async ({ page }) => {
    const systemServerCard = page
      .getByTestId(/^mcp-server-card-/)
      .filter({ hasText: 'Web Fetch' })
      .first()

    // System servers carry the System tag and expose no Edit affordance.
    await expect(systemServerCard.getByTestId('mcp-server-system-tag')).toBeVisible()
    await expect(systemServerCard.getByTestId('mcp-server-edit-btn')).toHaveCount(0)
  })

  test('should display empty state when no servers match filter', async ({ page }) => {
    // Search for non-existent server
    await byTestId(page, 'mcp-settings-search-input').fill('nonexistent-server-xyz')

    // Should display the empty state.
    await expect(byTestId(page, 'mcp-settings-empty')).toBeVisible()
  })

  // ──────────────────────────────────────────────────────────────────────
  // New coverage added for feat/mcp-rewrite-v2
  // ──────────────────────────────────────────────────────────────────────

  // Sort dropdown removed from the user MCP page by bcc2047 —
  // backend orders the list (`is_system ASC, display_name ASC`).
  test.skip('should default sort to "Date Added" on user MCP page', async () => {})

  test('should hide Delete on group-assigned system servers (read-only)', async ({ page }) => {
    // Web Fetch ships in the default group; verify it appears with no Delete button.
    const card = page
      .getByTestId(/^mcp-server-card-/)
      .filter({ hasText: 'Web Fetch' })
      .first()
    await expect(card).toBeVisible()
    await expect(card.getByTestId('mcp-server-delete-btn')).toHaveCount(0)
  })
})
