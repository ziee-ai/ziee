import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
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

test.describe('MCP - Admin System Servers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToMcpAdminPage(page, baseURL)
    await waitForMcpAdminPageLoad(page)
  })

  test('should pass accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('should display System MCP Servers page', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'System MCP Servers' })).toBeVisible()
    await expect(page.locator('text=Manage Model Context Protocol servers across the system')).toBeVisible()
    await expect(page.locator('button:has-text("Add Server")')).toBeVisible()
  })

  test('should display search and filter controls', async ({ page }) => {
    await expect(page.locator('input[placeholder="Search servers..."]')).toBeVisible()
    await expect(page.locator('.ant-select:has-text("All Servers")')).toBeVisible()
  })

  test('should display existing system servers', async ({ page }) => {
    // Default system servers from migration should be visible
    await expect(page.locator('.ant-card:has-text("Filesystem Access")')).toBeVisible()
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).toBeVisible()
  })

  test('should open Add System Server drawer', async ({ page }) => {
    await openAddServerDrawer(page, true)
    await expect(page.locator('.ant-drawer-title:has-text("Add System Server")')).toBeVisible()
    await expect(page.getByLabel('Name', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Display Name')).toBeVisible()
  })

  test('should create system HTTP server successfully', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-system-http',
      displayName: 'Test System HTTP',
      description: 'A test system HTTP MCP server',
      transportType: 'http',
      url: 'https://system-example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    // Verify success message
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  test('should create system stdio server successfully', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-system-stdio',
      displayName: 'Test System Stdio',
      description: 'A test system stdio MCP server',
      transportType: 'stdio',
      command: 'uvx',
      args: ['mcp-server-test'],
      env: { SYSTEM_VAR: 'system_value' },
      enabled: false, // Start disabled
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    // Verify success message
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  test('should edit system server', async ({ page }) => {
    // Edit an existing system server (Web Fetch)
    await clickEditServerButton(page, 'Web Fetch', true)

    // Verify drawer opens with pre-filled data
    await expect(page.locator('.ant-drawer-title:has-text("Edit System Server")')).toBeVisible()
    await expect(page.getByLabel('Display Name')).toHaveValue('Web Fetch')

    // Update description
    await page.getByLabel('Description').fill('Updated description for Web Fetch server')

    await submitMcpServerForm(page, 'update', true)

    // Verify success message
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })
  })

  test('should toggle system server enabled state', async ({ page }) => {
    // Toggle Filesystem Access server (starts disabled)
    await toggleServerEnabled(page, 'Filesystem Access')

    // Verify success message
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify switch state changed
    await verifyServerEnabled(page, 'Filesystem Access', true)
  })

  test('should filter system servers by search term', async ({ page }) => {
    const searchInput = page.locator('input[placeholder="Search servers..."]')
    await searchInput.fill('Filesystem')

    // Should show Filesystem server
    await expect(page.locator('.ant-card:has-text("Filesystem Access")')).toBeVisible()

    // Should not show Web Fetch server
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).not.toBeVisible()
  })

  test('should filter by enabled status', async ({ page }) => {
    // Click status filter
    await page.click('.ant-select:has-text("All Servers")')

    // Select 'Enabled' filter
    await page.click('.ant-select-item-option:has-text("Enabled")')

    // Should show only enabled servers
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).toBeVisible()

    // Should not show disabled servers
    await expect(page.locator('.ant-card:has-text("Filesystem Access")')).not.toBeVisible()
  })

  test('should filter by disabled status', async ({ page }) => {
    // Click status filter
    await page.click('.ant-select:has-text("All Servers")')

    // Select 'Disabled' filter
    await page.click('.ant-select-item-option:has-text("Disabled")')

    // Should show only disabled servers
    await expect(page.locator('.ant-card:has-text("Filesystem Access")')).toBeVisible()

    // Should not show enabled servers
    await expect(page.locator('.ant-card:has-text("Web Fetch")')).not.toBeVisible()
  })

  test('should sort servers by name', async ({ page }) => {
    // Sorting is already default (by name)
    const cards = page.locator('.ant-card')
    const firstCardText = await cards.first().textContent()

    // First card should be "Browser Automation" (alphabetically first)
    expect(firstCardText).toContain('Browser Automation')
  })

  test('should sort servers by status', async ({ page }) => {
    // Click sort dropdown
    await page.click('.ant-select:has-text("Name")')

    // Select 'Status' sort
    await page.click('.ant-select-item-option:has-text("Status")')

    await page.waitForTimeout(500)

    // Active servers should appear first
    const cards = page.locator('.ant-card')
    const count = await cards.count()

    if (count > 0) {
      const firstCardText = await cards.first().textContent()
      // Web Fetch is enabled, should be first
      expect(firstCardText).toContain('Web Fetch')
    }
  })

  test('should clear all filters', async ({ page }) => {
    // Apply search filter
    await page.fill('input[placeholder="Search servers..."]', 'test')

    // Apply status filter
    await page.click('.ant-select:has-text("All Servers")')
    await page.click('.ant-select-item-option:has-text("Enabled")')

    // Verify clear button appears
    await expect(page.locator('button:has-text("Clear all")')).toBeVisible()

    // Click clear all
    await page.click('button:has-text("Clear all")')

    // Verify filters are cleared
    await expect(page.locator('input[placeholder="Search servers..."]')).toHaveValue('')
  })

  test('should display all system servers as editable', async ({ page }) => {
    const serverCard = page.locator('.ant-card:has-text("Web Fetch")')

    // System servers in admin page should have Edit button
    await expect(serverCard.locator('button:has-text("Edit")')).toBeVisible()

    // Should have enabled/disabled switch
    await expect(serverCard.locator('.ant-switch')).toBeVisible()
  })

  test('should display empty state when no servers match filter', async ({ page }) => {
    // Search for non-existent server
    await page.fill('input[placeholder="Search servers..."]', 'nonexistent-admin-server-xyz')

    // Should display empty state
    await expect(page.locator('text=No servers match your search criteria')).toBeVisible()
  })

  test('should validate transport type cannot be changed in edit mode', async ({ page }) => {
    // Edit Web Fetch server
    await clickEditServerButton(page, 'Web Fetch', true)

    // Transport type dropdown should be disabled in edit mode
    // Find the Form.Item that contains "Transport Type" label and get its select
    const transportSelect = page.locator('.ant-form-item:has-text("Transport Type") .ant-select')

    // Check if the select is disabled
    const isDisabled = await transportSelect.evaluate((el) => {
      return el.classList.contains('ant-select-disabled')
    })

    // Transport type should be disabled in edit mode
    expect(isDisabled).toBe(true)
  })

  test('should create SSE transport server', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-sse-server',
      displayName: 'Test SSE Server',
      description: 'A test SSE MCP server',
      transportType: 'sse',
      url: 'https://example.com/mcp-sse',
      enabled: true,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    // Verify success message
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify server appears in list
    await verifyServerExists(page, serverData.displayName)
  })

  test('should create HTTP server with sampling enabled', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-sampling-http',
      displayName: 'Test Sampling HTTP',
      description: 'HTTP server with sampling enabled',
      transportType: 'http',
      url: 'https://sampling.example.com/mcp',
      enabled: true,
      supportsSampling: true,
      usageMode: 'always',
      maxConcurrentSessions: 3,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify sampling badge and always badge are visible on the card
    const serverCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(serverCard.locator('.ant-tag:has-text("Sampling")')).toBeVisible()
    await expect(serverCard.locator('.ant-tag:has-text("Always")')).toBeVisible()
  })

  test('should show no sampling badge when supports_sampling is false', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'test-no-sampling',
      displayName: 'Test No Sampling',
      transportType: 'http',
      url: 'https://nosampling.example.com/mcp',
      enabled: true,
      // supportsSampling defaults to false — no explicit value
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    const serverCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(serverCard.locator('.ant-tag:has-text("Sampling")')).not.toBeVisible()
    await expect(serverCard.locator('.ant-tag:has-text("Always")')).not.toBeVisible()
  })

  test('should preserve sampling fields when editing a server', async ({ page }) => {
    // First create a server with sampling enabled
    const serverData: McpServerFormData = {
      name: 'test-edit-sampling',
      displayName: 'Test Edit Sampling',
      transportType: 'http',
      url: 'https://editsampling.example.com/mcp',
      enabled: true,
      supportsSampling: true,
      usageMode: 'always',
      maxConcurrentSessions: 5,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Open edit drawer
    await clickEditServerButton(page, serverData.displayName, true)

    // Verify sampling fields are pre-filled
    const samplingSwitch = page.getByLabel('Enable MCP Sampling')
    const isSamplingChecked = await samplingSwitch.evaluate((el) =>
      el.classList.contains('ant-switch-checked')
    )
    expect(isSamplingChecked).toBe(true)

    await expect(page.getByLabel('Usage Mode')).toContainText('Always')
    await expect(page.getByLabel('Max Concurrent Sessions')).toHaveValue('5')
  })

  test('should update sampling settings via edit', async ({ page }) => {
    // Create server without sampling
    const serverData: McpServerFormData = {
      name: 'test-update-sampling',
      displayName: 'Test Update Sampling',
      transportType: 'http',
      url: 'https://updatesampling.example.com/mcp',
      enabled: true,
      supportsSampling: false,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify no sampling badges initially
    const serverCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(serverCard.locator('.ant-tag:has-text("Sampling")')).not.toBeVisible()

    // Edit and enable sampling
    await clickEditServerButton(page, serverData.displayName, true)
    await fillMcpServerForm(page, {
      ...serverData,
      supportsSampling: true,
      usageMode: 'always',
    })
    await submitMcpServerForm(page, 'update', true)

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })

    // Verify sampling and always badges now appear
    const updatedCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(updatedCard.locator('.ant-tag:has-text("Sampling")')).toBeVisible()
    await expect(updatedCard.locator('.ant-tag:has-text("Always")')).toBeVisible()
  })
})
