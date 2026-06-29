import { test, expect } from '../../fixtures/test-context'
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
  clickEditServerButton,
  type McpServerFormData,
} from './helpers/form-helpers'

// Phase 4: configuring per-server OAuth 2.1 (client_credentials) from the MCP
// server drawer. The client secret is write-only — never echoed back.

// The OAuth fields live behind an "Enable OAuth 2.1" toggle (only shown for
// HTTP user servers). In create mode the toggle starts OFF; in edit mode it
// auto-enables when the row already has an OAuth config. Expand it before
// touching the client-id/secret/scopes fields.
async function enableOAuthSection(page: import('@playwright/test').Page) {
  const toggle = byTestId(page, 'mcp-drawer-oauth-enabled-switch')
  await toggle.waitFor({ state: 'visible', timeout: 5000 })
  const isOn = (await toggle.getAttribute('aria-checked')) === 'true'
  if (!isOn) await toggle.click()
  await byTestId(page, 'mcp-drawer-oauth-client-id-input').waitFor({
    state: 'visible',
    timeout: 5000,
  })
}

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
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveCount(0)

    // Switch to HTTP → OAuth section appears. Use keyboard nav on the
    // combobox (option clicks are flaky for AntD Select in drawers).
    // Order: 0=Standard I/O, 1=HTTP, 2=Server-Sent Events.
    await byTestId(page, 'mcp-drawer-transport-select').click()
    await byTestId(page, 'mcp-drawer-transport-select-opt-http').click()

    // OAuth fields are gated behind the "Enable OAuth 2.1" toggle.
    await enableOAuthSection(page)
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toBeVisible()
    await expect(byTestId(page, 'mcp-drawer-oauth-secret-input')).toBeVisible()
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
    await enableOAuthSection(page)
    await byTestId(page, 'mcp-drawer-oauth-client-id-input').fill('mcp-client')
    await byTestId(page, 'mcp-drawer-oauth-secret-input').fill('super-secret')
    await byTestId(page, 'mcp-drawer-oauth-scopes-input').fill('mcp read')
    await submitMcpServerForm(page, 'create')

    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })
    await page.waitForTimeout(800)

    // Re-open in edit mode → the stored config loads.
    await clickEditServerButton(page, serverData.displayName)
    await expect(byTestId(page, 'mcp-drawer-name-input')).toHaveCount(0)

    // client_id + scopes are prefilled; the secret is NOT echoed — only a
    // "kept unless replaced" placeholder.
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveValue('mcp-client')
    await expect(byTestId(page, 'mcp-drawer-oauth-scopes-input')).toHaveValue('mcp read')
    await expect(byTestId(page, 'mcp-drawer-oauth-secret-input')).toHaveValue('')
    await expect(byTestId(page, 'mcp-drawer-oauth-secret-input')).toHaveAttribute(
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
    await enableOAuthSection(page)
    await byTestId(page, 'mcp-drawer-oauth-client-id-input').fill('mcp-client')
    await byTestId(page, 'mcp-drawer-oauth-secret-input').fill('super-secret')
    await submitMcpServerForm(page, 'create')
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })
    await page.waitForTimeout(800)

    // Edit → clear client id → save → reopen → config gone.
    await clickEditServerButton(page, serverData.displayName)
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveValue('mcp-client')
    await byTestId(page, 'mcp-drawer-oauth-client-id-input').fill('')
    await submitMcpServerForm(page, 'update')
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })
    await page.waitForTimeout(800)

    await clickEditServerButton(page, serverData.displayName)
    // Config was removed → the OAuth section collapses; re-expand to verify
    // the cleared (empty) state.
    await enableOAuthSection(page)
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveValue('')
    await expect(byTestId(page, 'mcp-drawer-oauth-secret-input')).toHaveAttribute(
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
    await enableOAuthSection(page)
    // Client id but no secret, and no existing config → must be rejected.
    await byTestId(page, 'mcp-drawer-oauth-client-id-input').fill('mcp-client')
    // Click directly (not the submit helper, which waits for the drawer to
    // close — here the OAuth step errors and the drawer deliberately stays
    // open). Submit label was standardised to verb-only "Create" (audit
    // I-2, commit b40cd8d) — scope to the open drawer's primary button.
    await byTestId(page, 'mcp-drawer-submit-btn').click()

    // The OAuth step rejects a client-id-without-secret: the save does NOT
    // complete, so the drawer deliberately stays open (deterministic signal).
    await expect(byTestId(page, 'mcp-drawer-form')).toBeVisible()
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
    await enableOAuthSection(page)
    await byTestId(page, 'mcp-drawer-oauth-client-id-input').fill('mcp-client')
    await byTestId(page, 'mcp-drawer-oauth-secret-input').fill('super-secret')
    await submitMcpServerForm(page, 'create')
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })
    await page.waitForTimeout(800)

    // Edit the display name; leave the secret field blank (= keep current).
    await clickEditServerButton(page, serverData.displayName)
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveValue('mcp-client')
    const renamed = 'OAuth Keep Server Renamed'
    await byTestId(page, 'mcp-drawer-display-name-input').fill(renamed)
    await submitMcpServerForm(page, 'update')
    await expect(byTestId(page, 'mcp-drawer-form')).toHaveCount(0, { timeout: 5000 })
    await page.waitForTimeout(800)

    // Reopen → OAuth config (and its secret) is still there: id prefilled and
    // the secret shows the "kept unless replaced" placeholder.
    await clickEditServerButton(page, renamed)
    await expect(byTestId(page, 'mcp-drawer-oauth-client-id-input')).toHaveValue('mcp-client')
    await expect(byTestId(page, 'mcp-drawer-oauth-secret-input')).toHaveAttribute(
      'placeholder',
      /unchanged/i,
    )
  })
})
