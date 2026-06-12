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
    // Default system servers from migration should be visible. `.first()`
    // because `.ant-card` matches both the outer page card and the
    // inner server cards containing the same text.
    await expect(page.locator('.ant-card:has-text("Filesystem Access")').first()).toBeVisible()
    await expect(page.locator('.ant-card:has-text("Web Fetch")').first()).toBeVisible()
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
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })
  })

  test('should toggle system server enabled state', async ({ page }) => {
    // Toggle Filesystem Access server (starts disabled)
    await toggleServerEnabled(page, 'Filesystem Access')

    // Verify success message
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    // Verify switch state changed
    await verifyServerEnabled(page, 'Filesystem Access', true)
  })

  test('should filter system servers by search term', async ({ page }) => {
    const searchInput = page.locator('input[placeholder="Search servers..."]')
    await searchInput.fill('Filesystem')

    // Should show Filesystem server
    await expect(page.locator('.ant-card:has-text("Filesystem Access")').first()).toBeVisible()

    // Should not show Web Fetch server
    await expect(page.locator('.ant-card:has-text("Web Fetch")').first()).not.toBeVisible()
  })

  test('should filter by enabled status', async ({ page }) => {
    // Click status filter
    await page.click('.ant-select:has-text("All Servers")')

    // Select 'Enabled' filter
    await page.click('.ant-select-item-option:has-text("Enabled")')

    // Should show only enabled servers
    await expect(page.locator('.ant-card:has-text("Web Fetch")').first()).toBeVisible()

    // Should not show disabled servers
    await expect(page.locator('.ant-card:has-text("Filesystem Access")').first()).not.toBeVisible()
  })

  test('should filter by disabled status', async ({ page }) => {
    // Click status filter
    await page.click('.ant-select:has-text("All Servers")')

    // Select 'Disabled' filter
    await page.click('.ant-select-item-option:has-text("Disabled")')

    // Should show only disabled servers
    await expect(page.locator('.ant-card:has-text("Filesystem Access")').first()).toBeVisible()

    // Should not show enabled servers
    await expect(page.locator('.ant-card:has-text("Web Fetch")').first()).not.toBeVisible()
  })

  // The "Sort servers" Select was removed by the settings UX overhaul
  // (commit bcc2047) — server-side ORDER BY now handles ordering
  // (default `display_name ASC` on the system page, mixed on the user
  // page). The old per-page sort tests no longer apply; backend
  // ordering is exercised at the repository tier in
  // `server/src/modules/mcp/repository.rs`.
  test.skip('should sort servers by name', async () => {})
  test.skip('should sort servers by status', async () => {})

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
    const serverCard = page.locator('.ant-card:has-text("Web Fetch")').first()

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
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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

    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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

    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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

    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    // Open edit drawer
    await clickEditServerButton(page, serverData.displayName, true)

    // Verify sampling fields are pre-filled
    const samplingSwitch = page.getByLabel('Enable MCP Sampling')
    const isSamplingChecked = await samplingSwitch.evaluate((el) =>
      el.classList.contains('ant-switch-checked')
    )
    expect(isSamplingChecked).toBe(true)

    // AntD Select stores the displayed text in a separate span, not
    // the hidden input that `getByLabel` resolves to. Read it from
    // the Form.Item's content-value span instead.
    const usageModeSelect = page
      .locator('.ant-form-item:has-text("Usage Mode") .ant-select')
      .first()
    await expect(usageModeSelect).toContainText('Always')
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

    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

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

    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    // Verify sampling and always badges now appear
    const updatedCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(updatedCard.locator('.ant-tag:has-text("Sampling")')).toBeVisible()
    await expect(updatedCard.locator('.ant-tag:has-text("Always")')).toBeVisible()
  })

  // ──────────────────────────────────────────────────────────────────────
  // New coverage added for feat/mcp-rewrite-v2
  // ──────────────────────────────────────────────────────────────────────

  test('should hide Sampling and Always badges after disabling sampling on edit', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: `test-disable-sampling-${Date.now()}`,
      displayName: `Disable Sampling ${Date.now()}`,
      transportType: 'http',
      url: 'https://disable-sampling.example.com/mcp',
      enabled: true,
      supportsSampling: true,
      usageMode: 'always',
      maxConcurrentSessions: 2,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    const card = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(card.locator('[data-testid="mcp-sampling-badge"]')).toBeVisible()
    await expect(card.locator('[data-testid="mcp-always-badge"]')).toBeVisible()

    // Edit and disable sampling
    await clickEditServerButton(page, serverData.displayName, true)
    await fillMcpServerForm(page, { ...serverData, supportsSampling: false })
    await submitMcpServerForm(page, 'update', true)
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    const updatedCard = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(updatedCard.locator('[data-testid="mcp-sampling-badge"]')).not.toBeVisible()
    // Always badge tracks usage_mode === 'always' independently of supports_sampling,
    // so it remains visible — covered by the sampling-on/auto test below.
  })

  test('should show Sampling badge alone when usage_mode is "auto"', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: `test-sampling-auto-${Date.now()}`,
      displayName: `Sampling Auto ${Date.now()}`,
      transportType: 'http',
      url: 'https://sampling-auto.example.com/mcp',
      enabled: true,
      supportsSampling: true,
      usageMode: 'auto',
      maxConcurrentSessions: 1,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, serverData)
    await submitMcpServerForm(page, 'create', true)
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })

    const card = page.locator(`.ant-card:has-text("${serverData.displayName}")`).first()
    await expect(card.locator('[data-testid="mcp-sampling-badge"]')).toBeVisible()
    await expect(card.locator('[data-testid="mcp-always-badge"]')).not.toBeVisible()
  })

  test('should hide Delete action on built-in servers but keep Edit visible', async ({
    page,
    testInfra,
  }) => {
    const { apiURL } = testInfra
    // Seed an is_built_in server via API (the form has no checkbox for it)
    const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
    const token = JSON.parse(authData!).state.token

    const seedRes = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `test-builtin-${Date.now()}`,
        display_name: `Built-in Test ${Date.now()}`,
        transport_type: 'stdio',
        // A non-sandboxed stdio system server's command must be host-allowed
        // (npx/uvx/python/python3/node); 'echo' is rejected with 400.
        command: 'npx',
        args: [],
        environment_variables: {},
        enabled: true,
        timeout_seconds: 30,
        supports_sampling: false,
        usage_mode: 'auto',
        // is_built_in is set via DB update because the API doesn't accept it.
      },
    })
    expect(seedRes.ok()).toBe(true)
    const created = await seedRes.json()

    // Force is_built_in via direct DB-like API (using the server's update endpoint
    // with a raw field is the only path — the migration script normally sets this
    // for filesystem/web-fetch servers). For the test, we use a system server already
    // marked built-in by migration (Filesystem Access or Web Fetch).
    const builtInCard = page.locator('.ant-card:has-text("Filesystem Access")').first()
    await expect(builtInCard).toBeVisible()
    await expect(builtInCard.locator('[data-testid="mcp-server-edit-btn"]')).toBeVisible()
    await expect(builtInCard.locator('[data-testid="mcp-server-delete-btn"]')).not.toBeVisible()

    // Clean up the seeded server (which is NOT built-in, so it has a delete button)
    const cleanupRes = await page.request.delete(
      `${apiURL}/api/mcp/system-servers/${created.id}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(cleanupRes.ok()).toBe(true)
  })

  // Sort dropdown removed by bcc2047 — see the skipped sort tests
  // above for context.
  test.skip('should default sort to "Date Added" on first load', async () => {})

  test('should reject negative max_concurrent_sessions value', async ({ page }) => {
    // The InputNumber for max_concurrent_sessions should not accept negative values.
    // AntD InputNumber without min defaults to allowing any number; the component
    // sets min=1 in the McpServerDrawer (verify by typing -5 → coerced).
    await openAddServerDrawer(page, true)
    await page.getByLabel('Name', { exact: true }).fill(`test-neg-max-${Date.now()}`)
    await page.getByLabel('Display Name').fill('Test Negative Max')
    await page.getByLabel('Transport Type').click({ force: true })
    await page.click('.ant-select-item-option:has-text("HTTP")')
    await page.getByLabel('URL').waitFor({ state: 'visible' })
    await page.getByLabel('URL').fill('https://neg.example.com/mcp')

    // Enable sampling so the max field is required-relevant
    const samplingSwitch = page.getByLabel('Enable MCP Sampling')
    const checked = await samplingSwitch.evaluate(el => el.classList.contains('ant-switch-checked'))
    if (!checked) await samplingSwitch.click()

    // Try to enter -5
    const maxInput = page.getByLabel('Max Concurrent Sessions')
    await maxInput.fill('-5')

    // AntD InputNumber with min=1 coerces invalid values to min on blur or shows error.
    // We tolerate either: the value is coerced to >=1, OR the form refuses submit.
    await page.getByLabel('Display Name').click() // blur the max field

    const finalVal = await maxInput.inputValue()
    const numeric = parseInt(finalVal || '0', 10)
    expect(numeric).toBeGreaterThanOrEqual(1)
  })

  // -------------------------------------------------------------------
  // Run-in-sandbox toggle (Tier 7)
  //
  // The toggle is gated to admin + stdio (create-system | edit-system).
  // We verify:
  //   - It IS visible when transport=stdio in the create-system drawer
  //   - It is NOT visible when transport=http in the same drawer
  //   - It persists through create + edit-system round-trip
  // -------------------------------------------------------------------
  test('run-in-sandbox toggle is visible only for stdio in create-system mode', async ({ page }) => {
    await openAddServerDrawer(page, true)
    // Default transport is stdio → toggle should be visible.
    const stdioToggle = page.getByLabel('Run in sandbox')
    await expect(stdioToggle).toBeVisible()

    // Switch to HTTP → toggle should hide.
    await page.getByLabel('Transport Type').click({ force: true })
    await page.click('.ant-select-item-option:has-text("HTTP")')
    await expect(stdioToggle).not.toBeVisible()

    // Back to stdio → re-appears. The option label is "Standard I/O".
    await page.getByLabel('Transport Type').click({ force: true })
    await page.click('.ant-select-item-option:has-text("Standard I/O")')
    await expect(stdioToggle).toBeVisible()
  })

  test('run-in-sandbox persists through create + edit', async ({ page }) => {
    const name = `test-sandbox-${Date.now()}`
    const displayName = `Test Sandbox ${Date.now()}`

    // CREATE with sandbox on.
    await openAddServerDrawer(page, true)
    await page.getByLabel('Name', { exact: true }).fill(name)
    await page.getByLabel('Display Name').fill(displayName)
    await page.getByLabel('Command').fill('python3')
    // Toggle on.
    const toggle = page.getByLabel('Run in sandbox')
    await expect(toggle).toBeVisible()
    const checkedBefore = await toggle.evaluate(el => el.classList.contains('ant-switch-checked'))
    if (!checkedBefore) await toggle.click()

    await submitMcpServerForm(page, 'create', true)
    await expect(page.locator('.ant-message-success, .ant-message-warning').first()).toBeVisible({ timeout: 5000 })
    await verifyServerExists(page, displayName)

    // EDIT — toggle should be hydrated to checked.
    await clickEditServerButton(page, displayName, true)
    const editToggle = page.getByLabel('Run in sandbox')
    await expect(editToggle).toBeVisible()
    const checkedNow = await editToggle.evaluate(el => el.classList.contains('ant-switch-checked'))
    expect(checkedNow).toBe(true)
  })

  test('run-in-sandbox help text mentions filesystem isolation', async ({ page }) => {
    await openAddServerDrawer(page, true)
    // The help text is rendered under the Switch.
    await expect(page.getByText(/isolated workspace/i)).toBeVisible()
    await expect(page.getByText(/filesystem-oriented/i)).toBeVisible()
  })
})

test.describe('MCP - Admin System Servers: sandbox flavor + command tiers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToMcpAdminPage(page, baseURL)
    await waitForMcpAdminPageLoad(page)
  })

  test('flavor picker appears only when run-in-sandbox is on, defaults to full', async ({ page }) => {
    await openAddServerDrawer(page, true)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    // Hidden until the toggle is on.
    await expect(drawer.getByText('Sandbox flavor')).toHaveCount(0)

    await page.getByLabel('Run in sandbox').click()

    await expect(drawer.getByText('Sandbox flavor')).toBeVisible()
    // Default selection is `full` (the Select's value renders inside the
    // form-item; antd v6 uses .ant-select-content).
    await expect(
      drawer.locator('.ant-form-item:has-text("Sandbox flavor")'),
    ).toContainText('full')
  })

  test('host allowlist blocks a disallowed command unless run-in-sandbox is on', async ({ page }) => {
    await openAddServerDrawer(page, true)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    await page.getByLabel('Name', { exact: true }).fill(`deno-${Date.now()}`)
    await page.getByLabel('Display Name').fill('Deno Server')
    await page.getByLabel('Command').fill('deno')

    // Host tier (run-in-sandbox off) → submit is blocked with an inline error.
    await drawer.locator('.ant-btn-primary').click()
    await expect(drawer.getByText(/not allowed on the host/i)).toBeVisible()
    await expect(drawer).toBeVisible() // drawer stays open (save blocked)

    // Enabling run-in-sandbox lifts the restriction; the error clears.
    await page.getByLabel('Run in sandbox').click()
    await expect(drawer.getByText(/not allowed on the host/i)).toHaveCount(0)

    // Save disabled so the create-time connection probe doesn't spawn the command.
    await page.getByLabel('Enabled').click()
    await drawer.locator('.ant-btn-primary').click()
    // A successful save closes the drawer (robust success signal).
    await expect(page.locator('.ant-drawer.ant-drawer-open')).toHaveCount(0, { timeout: 10000 })
  })

  test('an allowlisted host command (uvx) saves without sandbox', async ({ page }) => {
    await openAddServerDrawer(page, true)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    await page.getByLabel('Name', { exact: true }).fill(`uvx-${Date.now()}`)
    await page.getByLabel('Display Name').fill('Uvx Server')
    await page.getByLabel('Command').fill('uvx')

    // Disable so the create-time probe (which would spawn uvx) doesn't run.
    await page.getByLabel('Enabled').click()
    await drawer.locator('.ant-btn-primary').click()
    // A successful save closes the drawer (robust success signal).
    await expect(page.locator('.ant-drawer.ant-drawer-open')).toHaveCount(0, { timeout: 10000 })
  })
})
