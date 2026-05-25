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

    // Switch to HTTP → OAuth section appears. Use keyboard nav on the
    // combobox (option clicks are flaky for AntD Select in drawers).
    // Order: 0=Standard I/O, 1=HTTP, 2=Server-Sent Events.
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    const transportCombobox = drawer
      .locator('.ant-form-item:has-text("Transport Type")')
      .first()
      .getByRole('combobox')
    await transportCombobox.click({ force: true })
    await page.waitForTimeout(300)
    await transportCombobox.press('Home')
    await transportCombobox.press('ArrowDown')
    await transportCombobox.press('Enter')

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

    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })
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
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })
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

  test('blocks save when a client id is entered without a secret', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'oauth-validation-server',
      displayName: 'OAuth Validation Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    // Client id but no secret, and no existing config → must be rejected.
    await page.getByLabel('OAuth Client ID').fill('mcp-client')
    // Click directly (not the submit helper, which waits for the drawer to
    // close — here the OAuth step errors and the drawer deliberately stays open).
    await page.click('button:has-text("Create Server")')

    await expect(
      page.locator('.ant-message-error:has-text("client secret")'),
    ).toBeVisible({ timeout: 5000 })
    // The drawer stays open (save did not complete the OAuth step).
    await expect(
      page.locator('.ant-drawer-title:has-text("Add MCP Server")'),
    ).toBeVisible()
  })

  test('editing other fields keeps the stored secret', async ({ page }) => {
    const serverData: McpServerFormData = {
      name: 'oauth-keep-server',
      displayName: 'OAuth Keep Server',
      transportType: 'http',
      url: 'https://example.com/mcp',
      enabled: true,
    }

    await openAddServerDrawer(page)
    await fillMcpServerForm(page, serverData)
    await page.getByLabel('OAuth Client ID').fill('mcp-client')
    await page.getByLabel('OAuth Client Secret').fill('super-secret')
    await submitMcpServerForm(page, 'create')
    await expect(page.locator('.ant-message-success').first()).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(800)

    // Edit the display name; leave the secret field blank (= keep current).
    await clickEditServerButton(page, serverData.displayName)
    await expect(page.getByLabel('OAuth Client ID')).toHaveValue('mcp-client')
    const renamed = 'OAuth Keep Server Renamed'
    await page.getByLabel('Display Name').fill(renamed)
    await submitMcpServerForm(page, 'update')
    await expect(
      page.locator('.ant-message-success:has-text("updated")'),
    ).toBeVisible({ timeout: 5000 })
    await page.waitForTimeout(800)

    // Reopen → OAuth config (and its secret) is still there: id prefilled and
    // the secret shows the "kept unless replaced" placeholder.
    await clickEditServerButton(page, renamed)
    await expect(page.getByLabel('OAuth Client ID')).toHaveValue('mcp-client')
    await expect(page.getByLabel('OAuth Client Secret')).toHaveAttribute(
      'placeholder',
      /unchanged/i,
    )
  })
})
