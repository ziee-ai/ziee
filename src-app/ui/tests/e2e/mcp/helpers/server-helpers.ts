import { Page } from '@playwright/test'
import { byTestId } from '../../testid'
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
  await byTestId(page, 'mcp-system-add-btn').click()

  // Wait for the drawer form to render.
  await byTestId(page, 'mcp-drawer-form').waitFor({ state: 'visible', timeout: 5000 })

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
  // Find the system server card by its dynamic display name.
  const serverCard = page
    .getByTestId(/^mcp-system-server-card-/)
    .filter({ hasText: serverDisplayName })
    .first()
  await serverCard.waitFor({ state: 'visible', timeout: 10000 })

  // A server must be DISABLED before it can be deleted — the delete Button is
  // `disabled={server.enabled}` (with a "Disable the server before deleting it"
  // tooltip). Turn the enable switch off first if it's on; the toggle's API
  // call flips `enabled` false and re-enables the delete button (the `.click()`
  // below auto-waits for that).
  const enableSwitch = byTestId(serverCard, 'mcp-server-enable-switch')
  if (
    (await enableSwitch.count()) > 0 &&
    (await enableSwitch.getAttribute('aria-checked')) === 'true'
  ) {
    await enableSwitch.click()
  }

  // Click delete (scoped to the card) → confirm in the dialog.
  const deleteButton = byTestId(serverCard, 'mcp-server-delete-btn')
  if (await deleteButton.count() > 0) {
    await deleteButton.click()
    await byTestId(page, 'mcp-server-delete-confirm-confirm').click()
    // Deletion is confirmed by the card leaving the DOM.
    await serverCard.waitFor({ state: 'detached', timeout: 10000 })
  }
}
