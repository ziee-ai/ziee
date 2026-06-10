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
  // sampling fields
  supportsSampling?: boolean
  usageMode?: 'auto' | 'always'
  maxConcurrentSessions?: number
}

export async function openAddServerDrawer(page: Page, _isSystemServer = false) {
  await page.click('button:has-text("Add Server")')
  // Wait for drawer to be fully open
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible', timeout: 5000 })
  // Wait for the first form field to be ready using semantic selector
  await page.getByLabel('Display Name').waitFor({ state: 'visible', timeout: 5000 })
}

export async function fillMcpServerForm(page: Page, data: McpServerFormData) {
  // Scope label lookups to the active drawer because AntD leaves
  // closed drawers in the DOM and `page.getByLabel` would otherwise
  // resolve to both the closed Create drawer's fields and the open
  // Edit drawer's fields (strict-mode flake).
  const drawer = page.locator('.ant-drawer.ant-drawer-open')

  // Name field (only visible in create mode)
  const nameField = drawer.getByLabel('Name', { exact: true })
  if (await nameField.isVisible()) {
    await nameField.fill(data.name)
  }

  // Display name
  await drawer.getByLabel('Display Name').fill(data.displayName)

  // Description (optional)
  if (data.description) {
    await drawer.getByLabel('Description').fill(data.description)
  }

  // Transport type — use keyboard nav. UI order (per
  // McpServerDrawer.tsx TRANSPORT_TYPES) is:
  //   0: stdio (Standard I/O), 1: http (HTTP), 2: sse (Server-Sent Events)
  const transportCombobox = drawer
    .locator('.ant-form-item:has-text("Transport Type")')
    .first()
    .getByRole('combobox')
  await transportCombobox.click({ force: true })
  await page.waitForTimeout(300)
  const transportIdx = data.transportType === 'stdio' ? 0 : data.transportType === 'http' ? 1 : 2
  await transportCombobox.press('Home')
  for (let i = 0; i < transportIdx; i++) {
    await transportCombobox.press('ArrowDown')
  }
  await transportCombobox.press('Enter')

  // Wait for transport-specific fields to appear and be ready
  if (data.transportType === 'stdio') {
    // Wait for command field to be visible
    await drawer.getByLabel('Command').waitFor({ state: 'visible', timeout: 5000 })

    // Command
    if (data.command) {
      await drawer.getByLabel('Command').fill(data.command)
    }

    // Arguments (JSON array)
    if (data.args) {
      await drawer.getByLabel('Arguments').fill(JSON.stringify(data.args))
    }

    // Environment variables. The field is now a KeyValueSecretEditor
    // (a "+ Add env var" button + one key/value row per entry), not a
    // JSON textarea. Add one row per variable and fill the key + value
    // inputs by their placeholders ("GITHUB_TOKEN" / "value").
    if (data.env) {
      for (const [key, value] of Object.entries(data.env)) {
        await drawer.getByRole('button', { name: /Add env var/i }).click()
        await drawer
          .locator('input[placeholder="GITHUB_TOKEN"]')
          .last()
          .fill(key)
        await drawer
          .locator('input[placeholder="value"]')
          .last()
          .fill(String(value))
      }
    }
  } else {
    // Wait for URL field to be visible and ready
    await drawer.getByLabel('URL').waitFor({ state: 'visible', timeout: 5000 })

    // URL for http/sse
    if (data.url) {
      await drawer.getByLabel('URL').fill(data.url)
    }
  }

  // Enabled switch (first switch in the form)
  if (data.enabled !== undefined) {
    const enabledSwitch = drawer.getByLabel('Enabled')
    const isChecked = await enabledSwitch.evaluate((el) =>
      el.classList.contains('ant-switch-checked')
    )
    if (isChecked !== data.enabled) {
      await enabledSwitch.click()
    }
  }

  // Sampling fields
  if (data.supportsSampling !== undefined) {
    const samplingSwitch = drawer.getByLabel('Enable MCP Sampling')
    const isChecked = await samplingSwitch.evaluate((el) =>
      el.classList.contains('ant-switch-checked')
    )
    if (isChecked !== data.supportsSampling) {
      await samplingSwitch.click()
    }
  }

  if (data.usageMode !== undefined) {
    // Usage Mode only renders when sampling is enabled — skip if the
    // field isn't present (e.g. when disabling sampling).
    const usageItem = drawer
      .locator('.ant-form-item:has-text("Usage Mode")')
      .first()
    if (await usageItem.isVisible().catch(() => false)) {
      // Keyboard nav. Options: 0: Auto, 1: Always.
      const usageCombobox = usageItem.getByRole('combobox')
      await usageCombobox.click({ force: true })
      await page.waitForTimeout(300)
      const usageIdx = data.usageMode === 'always' ? 1 : 0
      await usageCombobox.press('Home')
      for (let i = 0; i < usageIdx; i++) {
        await usageCombobox.press('ArrowDown')
      }
      await usageCombobox.press('Enter')
    }
  }

  if (data.maxConcurrentSessions !== undefined) {
    // Max Concurrent Sessions, like Usage Mode, only renders when
    // sampling is enabled — skip if it isn't present.
    const maxField = drawer.getByLabel('Max Concurrent Sessions')
    if (await maxField.isVisible().catch(() => false)) {
      await maxField.fill(String(data.maxConcurrentSessions))
    }
  }
}

export async function submitMcpServerForm(page: Page, action: 'create' | 'update' = 'create', isSystemServer = false) {
  // Submit labels were standardised to verb-only ("Create Server" →
  // "Create", "Update Server" → "Save", audit I-2). Scope the click
  // to the open drawer's primary button rather than text-matching.
  const drawerTitle = isSystemServer
    ? (action === 'create' ? 'Add System Server' : 'Edit System Server')
    : (action === 'create' ? 'Add MCP Server' : 'Edit MCP Server')

  await page.locator('.ant-drawer.ant-drawer-open').last().locator('.ant-btn-primary').click()

  // Wait for success OR auto-disabled warning toast (Ant Design
  // auto-dismisses after ~3s, so this must happen before checking
  // drawer-closed). connection_health probes enabled creates and
  // downgrades to warning when the URL is unreachable — that's still
  // a successful round-trip from the test's perspective.
  await page.waitForSelector(
    '.ant-message-success, .ant-message-warning',
    { state: 'visible', timeout: 5000 },
  )

  // Wait for specific drawer to close by waiting for its title to disappear
  await page.waitForSelector(`.ant-drawer-title:has-text("${drawerTitle}")`, {
    state: 'hidden',
    timeout: 10000
  })
}

export async function clickEditServerButton(page: Page, serverName: string, isSystemServer = false) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`).first()
  await serverCard.locator('button:has-text("Edit")').click()
  // Wait for specific Edit drawer title, not generic .ant-drawer-open class
  const drawerTitle = isSystemServer ? 'Edit System Server' : 'Edit MCP Server'
  await page.waitForSelector(`.ant-drawer-title:has-text("${drawerTitle}")`, { timeout: 5000 })
}

export async function deleteServer(_page: Page, _serverName: string) {
  // This function would be for delete functionality if/when implemented
  // For now, just a placeholder
}

export async function verifyServerExists(page: Page, serverName: string) {
  await expect(page.locator(`.ant-card:has-text("${serverName}")`).first()).toBeVisible()
}

export async function verifyServerNotExists(page: Page, serverName: string) {
  await expect(page.locator(`.ant-card:has-text("${serverName}")`)).not.toBeVisible()
}

export async function toggleServerEnabled(page: Page, serverName: string) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`).first()
  const switchButton = serverCard.locator('.ant-switch').first()
  await switchButton.click()
  // Enable-toggle runs a connection-health probe; success path =
  // success toast, probe-failure path = warning toast that
  // auto-disables. Either means the round-trip happened.
  await page.waitForSelector(
    '.ant-message-success, .ant-message-warning',
    { state: 'visible', timeout: 5000 },
  )
}

export async function verifyServerEnabled(page: Page, serverName: string, enabled: boolean) {
  const serverCard = page.locator(`.ant-card:has-text("${serverName}")`).first()
  const switchButton = serverCard.locator('.ant-switch').first()
  const isChecked = await switchButton.evaluate((el) =>
    el.classList.contains('ant-switch-checked')
  )
  expect(isChecked).toBe(enabled)
}
