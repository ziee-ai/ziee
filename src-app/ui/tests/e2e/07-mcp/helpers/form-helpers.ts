import { Page, Locator, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * MCP-specific form helpers (kit/testid-based).
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

/** True when a kit Switch (radix) is in the checked state. */
async function switchIsChecked(sw: Locator): Promise<boolean> {
  return (await sw.getAttribute('aria-checked')) === 'true'
}

/** Locate a server card (user or system page) by its dynamic display name. */
function serverCardByName(page: Page, serverName: string, isSystemServer = false): Locator {
  const prefix = isSystemServer ? /^mcp-system-server-card-/ : /^mcp-server-card-/
  return page.getByTestId(prefix).filter({ hasText: serverName }).first()
}

export async function openAddServerDrawer(page: Page, isSystemServer = false) {
  await byTestId(page, isSystemServer ? 'mcp-system-add-btn' : 'mcp-settings-add-btn').click()
  // Wait for the drawer form + its first field to be ready.
  await byTestId(page, 'mcp-drawer-form').waitFor({ state: 'visible', timeout: 5000 })
  await byTestId(page, 'mcp-drawer-display-name-input').waitFor({ state: 'visible', timeout: 5000 })
}

export async function fillMcpServerForm(page: Page, data: McpServerFormData) {
  // Scope field lookups to the open drawer form.
  const drawer = byTestId(page, 'mcp-drawer-form')

  // Name field (only visible in create mode)
  const nameField = byTestId(drawer, 'mcp-drawer-name-input')
  if (await nameField.isVisible().catch(() => false)) {
    await nameField.fill(data.name)
  }

  // Display name
  await byTestId(drawer, 'mcp-drawer-display-name-input').fill(data.displayName)

  // Description (optional)
  if (data.description) {
    await byTestId(drawer, 'mcp-drawer-description-textarea').fill(data.description)
  }

  // Transport type — open the kit Select and pick the option by its derived
  // `${testid}-opt-${value}` id (options render in a portal, so scope to page).
  const transportSelect = byTestId(drawer, 'mcp-drawer-transport-select')
  await transportSelect.scrollIntoViewIfNeeded()
  await transportSelect.click()
  await byTestId(page, `mcp-drawer-transport-select-opt-${data.transportType}`).click()

  // Wait for transport-specific fields to appear and be ready
  if (data.transportType === 'stdio') {
    await byTestId(drawer, 'mcp-drawer-command-input').waitFor({ state: 'visible', timeout: 5000 })

    if (data.command) {
      await byTestId(drawer, 'mcp-drawer-command-input').fill(data.command)
    }

    if (data.args) {
      await byTestId(drawer, 'mcp-drawer-args-textarea').fill(JSON.stringify(data.args))
    }

    // Environment variables — a KeyValueSecretEditor: click "Add env var" then
    // fill the newest key/value row (derived `mcp-kv-environment_variables_entries-*`).
    if (data.env) {
      for (const [key, value] of Object.entries(data.env)) {
        await byTestId(drawer, 'mcp-kv-environment_variables_entries-add-btn').click()
        await drawer
          .getByTestId(/^mcp-kv-environment_variables_entries-key-/)
          .last()
          .fill(key)
        await drawer
          .getByTestId(/^mcp-kv-environment_variables_entries-value-/)
          .last()
          .fill(String(value))
      }
    }
  } else {
    await byTestId(drawer, 'mcp-drawer-url-input').waitFor({ state: 'visible', timeout: 5000 })
    if (data.url) {
      await byTestId(drawer, 'mcp-drawer-url-input').fill(data.url)
    }
  }

  // Enabled switch
  if (data.enabled !== undefined) {
    const enabledSwitch = byTestId(drawer, 'mcp-drawer-enabled-switch')
    if ((await switchIsChecked(enabledSwitch)) !== data.enabled) {
      await enabledSwitch.scrollIntoViewIfNeeded()
      await enabledSwitch.click()
    }
  }

  // Sampling fields
  if (data.supportsSampling !== undefined) {
    const samplingSwitch = byTestId(drawer, 'mcp-drawer-sampling-switch')
    if ((await switchIsChecked(samplingSwitch)) !== data.supportsSampling) {
      await samplingSwitch.scrollIntoViewIfNeeded()
      await samplingSwitch.click()
    }
  }

  if (data.usageMode !== undefined) {
    // Usage Mode only renders when sampling is enabled — skip if absent.
    const usageSelect = byTestId(drawer, 'mcp-drawer-usage-mode-select')
    if (await usageSelect.isVisible().catch(() => false)) {
      await usageSelect.click()
      await byTestId(page, `mcp-drawer-usage-mode-select-opt-${data.usageMode}`).click()
    }
  }

  if (data.maxConcurrentSessions !== undefined) {
    // Max Concurrent Sessions also only renders when sampling is enabled.
    const maxField = byTestId(drawer, 'mcp-drawer-max-sessions-input')
    if (await maxField.isVisible().catch(() => false)) {
      await maxField.fill(String(data.maxConcurrentSessions))
    }
  }
}

export async function submitMcpServerForm(
  page: Page,
  _action: 'create' | 'update' = 'create',
  _isSystemServer = false,
) {
  await byTestId(page, 'mcp-drawer-submit-btn').click()
  // A successful submit (including the auto-disabled-warning round-trip, which
  // still persists) closes the drawer — wait for the form to leave the DOM.
  await byTestId(page, 'mcp-drawer-form').waitFor({ state: 'detached', timeout: 10000 })
}

export async function clickEditServerButton(page: Page, serverName: string, isSystemServer = false) {
  const serverCard = serverCardByName(page, serverName, isSystemServer)
  await byTestId(serverCard, 'mcp-server-edit-btn').click()
  // Wait for the edit drawer form to render.
  await byTestId(page, 'mcp-drawer-form').waitFor({ state: 'visible', timeout: 5000 })
}

export async function deleteServer(_page: Page, _serverName: string) {
  // Placeholder — delete-from-card lives in server-helpers.deleteSystemServer.
}

export async function verifyServerExists(page: Page, serverName: string) {
  await expect(serverCardByName(page, serverName)).toBeVisible()
}

export async function verifyServerNotExists(page: Page, serverName: string) {
  await expect(serverCardByName(page, serverName)).toHaveCount(0)
}

export async function toggleServerEnabled(page: Page, serverName: string) {
  const serverCard = serverCardByName(page, serverName)
  const switchButton = byTestId(serverCard, 'mcp-server-enable-switch')
  // The enable-toggle runs a server-side connection-health round-trip; wait on
  // the PUT response so the helper returns only after it completes (replaces the
  // old success/warning toast wait).
  const roundTrip = page.waitForResponse(
    r => /\/api\/mcp\/.*servers\//.test(r.url()) && r.request().method() === 'PUT',
    { timeout: 10000 },
  )
  await switchButton.click()
  await roundTrip
}

export async function verifyServerEnabled(page: Page, serverName: string, enabled: boolean) {
  const serverCard = serverCardByName(page, serverName)
  const switchButton = byTestId(serverCard, 'mcp-server-enable-switch')
  expect(await switchIsChecked(switchButton)).toBe(enabled)
}
