/**
 * MCP create-mode Enable switch — probes the form values WITHOUT
 * persisting a server. Mirror of the LLM-repository spec #6 pattern.
 *
 *   - Open Add MCP Server drawer (HTTP transport — no subprocess
 *     needed, keeps the test self-contained)
 *   - Fill required fields with URL pointing at a failing mock
 *   - Flip Enable switch ON
 *   - Assert: error toast, switch snaps back OFF, no server was created
 *     (verify via API count before + after)
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from './helpers/navigation-helpers'
// The LLM-repo health mock is a tiny status-only HTTP responder — it
// works just as well for an HTTP MCP server's initialize handshake
// since the probe expects a non-200 to be a failure. We get a 401 on
// the initial POST, which the MCP test path surfaces as
// "Unauthorized" via its existing error mapping.
import { RepoHealthMock } from '../05-llm/helpers/repository-health-mock'

async function userMcpServerCount(
  apiURL: string,
  token: string,
): Promise<number> {
  const resp = await fetch(`${apiURL}/api/mcp/servers`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!resp.ok) throw new Error(`list failed: ${resp.status}`)
  const body = await resp.json()
  // The MCP user list returns an array of accessible servers; system
  // ones are folded in too. Filter to user-owned (not is_system) for
  // a clean delta.
  const servers = Array.isArray(body) ? body : body.servers ?? []
  return servers.filter((s: any) => !s.is_system).length
}

test('Add MCP Server drawer: Enable switch tests the form WITHOUT persisting on failure', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(401)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const before = await userMcpServerCount(baseURL, token)

    await loginAsAdmin(page, baseURL)
    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)

    // Open the Add MCP Server drawer.
    await page.click('button:has-text("Add Server")')
    await page.waitForSelector(
      '.ant-drawer-title:has-text("Add MCP Server")',
      { timeout: 10_000 },
    )
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    // Fill required fields. Drawer defaults to HTTP transport for new
    // user servers (see McpServerDrawer.tsx); just need name + URL.
    const name = `create-test-mcp-${Math.random().toString(36).slice(2, 8)}`
    await drawer.getByLabel('Display Name').fill(name)
    // Name slug allows only [a-z0-9-]; `name` already uses hyphens, so don't
    // convert to underscores (which would fail the form's pattern validator
    // and silently block the toggle-ON connection probe).
    await drawer.getByLabel('Name', { exact: true }).fill(name)
    // Switch to HTTP transport if not already there.
    const transportCombobox = drawer.getByLabel('Transport Type')
    await transportCombobox.click()
    await page.locator('.ant-select-item-option:has-text("HTTP")').first().click()
    await drawer.getByLabel('URL').fill(mock.url())

    // The Enable switch is on the drawer title (added in the recent
    // health-check work). In CREATE mode it defaults to ON (mirrors the
    // form's `enabled`) without having probed anything; toggling it ON
    // runs an ephemeral connection-test against the form values (toggling
    // OFF is purely local). Toggle OFF then ON to fire the probe.
    const titleSwitch = drawer.locator(
      '.ant-drawer-header button.ant-switch',
    )
    await expect(titleSwitch).toHaveAttribute('aria-checked', 'true')
    await titleSwitch.click() // OFF — local only, no probe
    await expect(titleSwitch).toHaveAttribute('aria-checked', 'false')

    // Toggle ON — probe should fire against the 401 mock.
    await titleSwitch.click()

    // Error toast surfaces the failure.
    await expect(
      page.locator('.ant-message-error').first(),
    ).toBeVisible({ timeout: 30_000 })

    // Switch snaps back OFF.
    await expect(titleSwitch).toHaveAttribute('aria-checked', 'false', {
      timeout: 10_000,
    })

    // No row was created.
    const after = await userMcpServerCount(baseURL, token)
    expect(after).toBe(before)
  } finally {
    await mock.dispose()
  }
})
