import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from './helpers/navigation-helpers'
import {
  openAddServerDrawer,
  fillMcpServerForm,
  submitMcpServerForm,
  clickEditServerButton,
  type McpServerFormData,
} from './helpers/form-helpers'

/**
 * E2E — an SSE-transport MCP server round-trips its transport type + URL.
 *
 * Audit gap: `mcp-admin-servers.spec.ts` creates an SSE server but only
 * asserts it appears in the list — it never verifies the SSE-specific
 * config persisted. This re-opens the created server's Edit drawer and
 * asserts the Transport Type shows "Server-Sent Events" and the SSE URL
 * was stored (proving the transport_type/url columns round-trip the DB).
 */

test.describe('MCP — SSE transport persistence', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToMcpAdminPage(page, testInfra.baseURL)
    await waitForMcpAdminPageLoad(page)
  })

  test('SSE transport type + URL persist and show in the edit drawer', async ({
    page,
  }) => {
    const data: McpServerFormData = {
      name: `sse-persist-${Date.now()}`,
      displayName: `SSE Persist ${Date.now()}`,
      description: 'SSE transport persistence E2E',
      transportType: 'sse',
      url: 'https://example.com/sse-endpoint',
      enabled: false,
    }

    await openAddServerDrawer(page, true)
    await fillMcpServerForm(page, data)
    await submitMcpServerForm(page, 'create', true)
    await expect(
      page.locator('.ant-message-success, .ant-message-warning').first(),
    ).toBeVisible({ timeout: 5000 })

    // Re-open the created server's Edit drawer and assert the SSE config
    // persisted.
    await clickEditServerButton(page, data.displayName, true)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    await expect(
      drawer.locator('.ant-form-item:has-text("Transport Type")'),
    ).toContainText('Server-Sent Events')
    await expect(drawer.getByLabel('URL')).toHaveValue(data.url!)
  })
})
