import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from '../mcp/helpers/navigation-helpers'
import {
  toggleServerEnabled,
  verifyServerEnabled,
} from '../mcp/helpers/form-helpers'

/**
 * E2E — the built-in App Control MCP server is a VISIBLE, admin-toggleable
 * built-in on the System MCP page (like bio_mcp), so an admin can enable/disable
 * it without a config change or restart. No real LLM needed — pure admin UI.
 *
 * The runtime auto-attach honors the row's `enabled` column at both the
 * tools/list and execute sites, so toggling it off here stops the control tools
 * from being offered to the chat model.
 */

const CONTROL_DISPLAY_NAME = 'App Control'

test.describe('control_mcp — admin enable/disable on the System MCP page', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToMcpAdminPage(page, testInfra.baseURL)
    await waitForMcpAdminPageLoad(page)
  })

  test('control is listed and starts enabled', async ({ page }) => {
    // It must appear on the System MCP page (not hidden) and be on by default.
    await expect(page.getByText(CONTROL_DISPLAY_NAME, { exact: true })).toBeVisible()
    await verifyServerEnabled(page, CONTROL_DISPLAY_NAME, true)
  })

  test('admin can toggle control off and back on; the change persists', async ({
    page,
    testInfra,
  }) => {
    // Disable.
    await toggleServerEnabled(page, CONTROL_DISPLAY_NAME)
    await verifyServerEnabled(page, CONTROL_DISPLAY_NAME, false)

    // Persisted across a reload (the row's enabled column, not ephemeral UI state).
    await goToMcpAdminPage(page, testInfra.baseURL)
    await waitForMcpAdminPageLoad(page)
    await verifyServerEnabled(page, CONTROL_DISPLAY_NAME, false)

    // Re-enable round-trips too.
    await toggleServerEnabled(page, CONTROL_DISPLAY_NAME)
    await verifyServerEnabled(page, CONTROL_DISPLAY_NAME, true)
  })
})
