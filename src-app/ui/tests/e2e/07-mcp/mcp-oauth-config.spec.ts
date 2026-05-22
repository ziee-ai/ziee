import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from './helpers/navigation-helpers'
import {
  openAddServerDrawer,
  fillMcpServerForm,
  submitMcpServerForm,
  clickEditServerButton,
  type McpServerFormData,
} from './helpers/form-helpers'

// Phase 4: configuring per-server OAuth 2.1 (client_credentials) from the MCP
// server drawer. The client secret is write-only — never echoed back.
test.describe('MCP - OAuth config', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)
  })

  test('OAuth fields appear only for HTTP transport', async ({ page }) => {
    await openAddServerDrawer(page)
    // Default transport is stdio → no OAuth section.
    await expect(page.getByLabel('OAuth Client ID')).toHaveCount(0)

    // Switch to HTTP → OAuth section appears.
    await page.getByLabel('Transport Type').click({ force: true })
    await page.locator('.ant-select-item-option:has-text("HTTP")').first().click()
    await expect(page.getByLabel('OAuth Client ID')).toBeVisible()
    await expect(page.getByLabel('OAuth Client Secret')).toBeVisible()
  })

  test('create with OAuth, then edit shows id prefilled + secret kept', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'oauth-http-server',
      displayName: 'OAuth HTTP Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await page.getByLabel('OAuth Client ID').fill('mcp-client')
    await page.getByLabel('OAuth Client Secret').fill('super-secret')
    await page.getByLabel('OAuth Scopes').fill('mcp read')
    await submitMcpServerForm(page, 'create')

    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(800)

    // Re-open in edit mode → the stored config loads.
    await clickEditServerButton(page, serverData.displayName)
    await expect(
      page.locator('.ant-drawer-title:has-text("Edit MCP Server")'),
    ).toBeVisible()

    // client_id + scopes are prefilled; the secret is NOT echoed — only a
    // "kept unless replaced" placeholder.
    await expect(page.getByLabel('OAuth Client ID')).toHaveValue('mcp-client')
    await expect(page.getByLabel('OAuth Scopes')).toHaveValue('mcp read')
    await expect(page.getByLabel('OAuth Client Secret')).toHaveValue('')
    await expect(page.getByLabel('OAuth Client Secret')).toHaveAttribute(
      'placeholder',
      /unchanged/i,
    )
  })

  test('clearing the client id removes the OAuth config', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'oauth-remove-server',
      displayName: 'OAuth Remove Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await page.getByLabel('OAuth Client ID').fill('mcp-client')
    await page.getByLabel('OAuth Client Secret').fill('super-secret')
    await submitMcpServerForm(page, 'create')
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(800)

    // Edit → clear client id → save → reopen → config gone.
    await clickEditServerButton(page, serverData.displayName)
    await expect(page.getByLabel('OAuth Client ID')).toHaveValue('mcp-client')
    await page.getByLabel('OAuth Client ID').fill('')
    await submitMcpServerForm(page, 'update')
    await expect(
      page.locator('.ant-message-success:has-text("updated")'),
    ).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(800)

    await clickEditServerButton(page, serverData.displayName)
    await expect(page.getByLabel('OAuth Client ID')).toHaveValue('')
    await expect(page.getByLabel('OAuth Client Secret')).toHaveAttribute(
      'placeholder',
      /client secret/i,
    )
  })
})
