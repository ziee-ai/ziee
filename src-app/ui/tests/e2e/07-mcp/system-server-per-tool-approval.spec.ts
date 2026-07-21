import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from '../mcp/helpers/navigation-helpers'
import { clickEditServerButton } from '../mcp/helpers/form-helpers'

/**
 * TEST-189 (ITEM-55) — the ADMIN per-tool approval surface (positive control to
 * the A10 negative-perm spec TEST-190).
 *
 * asserts (task reframe): an admin opens the system MCP server drawer → "Tool
 * approvals" tab → sees the per-tool approval controls (Auto-approve / Manual
 * approve / Disabled) and SETS a tool's approval mode. The dead loopback URL makes
 * the live `tools/list` probe fail, exercising the `tools_unreachable` state and
 * the override-keyed ("cached") tool fallback: a tool that already carries an admin
 * override is still listed + editable when the server is unreachable.
 *
 * NOTE (reported): the shipped `McpToolApprovalsTab` does NOT implement the
 * "set-all", the ListPagination "Showing N of M", or the "external stricter hint
 * at Auto" that the TESTS.md TEST-189 assert names — those are not present in the
 * component, so this spec proves the built surface (tool list + modes + set).
 */
test.describe('System MCP per-tool approvals — admin (ITEM-55)', () => {
  test('admin opens the Tool approvals tab and sets a tool approval mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A system server (dead loopback URL → tools/list unreachable).
    const suffix = Date.now().toString(36)
    const name = `approval-admin-${suffix}`
    const displayName = `Approval Admin ${suffix}`
    const createRes = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name,
        display_name: displayName,
        description: 'Fixture system server for the per-tool approval A10 spec',
        transport_type: 'http',
        url: 'http://127.0.0.1:9/mcp',
        enabled: false,
      },
    })
    expect(createRes.ok()).toBe(true)
    const serverId = (await createRes.json()).id as string

    // Seed a per-tool override so a tool row is listed even while the server is
    // unreachable (the override-keyed / "cached" fallback the assert names).
    const seedRes = await page.request.put(
      `${apiURL}/api/mcp/servers/${serverId}/tool-approvals/search`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: { mode: 'auto_approve' },
      },
    )
    expect(seedRes.ok()).toBe(true)

    // Open the server drawer in edit-system mode → Tool approvals tab.
    await goToMcpAdminPage(page, baseURL)
    await waitForMcpAdminPageLoad(page)
    await clickEditServerButton(page, displayName, true)
    await expect(byTestId(page, 'mcp-drawer-tabs')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'mcp-drawer-tabs-tab-tool-approvals').click()

    // The admin approval card renders …
    await expect(byTestId(page, 'mcp-tool-approvals-card')).toBeVisible({
      timeout: 15000,
    })
    // … the unreachable state is surfaced (not a silent empty list) …
    await expect(byTestId(page, 'mcp-tool-approvals-unreachable')).toBeVisible({
      timeout: 15000,
    })
    // … and the override-keyed tool is listed with its current mode (Auto-approve).
    const select = byTestId(page, 'mcp-tool-approval-select-search')
    await expect(select).toBeVisible({ timeout: 15000 })
    await expect(select).toContainText('Auto-approve')

    // Set the tool's approval mode → Manual approve. The three admin modes are the
    // Select options (Auto-approve / Manual approve / Disabled).
    await select.click()
    await expect(
      byTestId(page, 'mcp-tool-approval-select-search-opt-auto_approve'),
    ).toBeVisible()
    await expect(
      byTestId(page, 'mcp-tool-approval-select-search-opt-disabled'),
    ).toBeVisible()
    const [putResp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/mcp\/servers\/[0-9a-f-]+\/tool-approvals\/search$/.test(r.url()) &&
          r.request().method() === 'PUT',
        { timeout: 15000 },
      ),
      byTestId(page, 'mcp-tool-approval-select-search-opt-manual_approve').click(),
    ])
    expect(putResp.ok()).toBeTruthy()

    // The Select reflects the new mode (the component folds effective_mode back).
    await expect(select).toContainText('Manual approve', { timeout: 10000 })

    // Renders at a 390px mobile width. Resizing reflows the drawer (which resets
    // its active tab), so re-select Tool approvals, then confirm the card renders.
    await page.setViewportSize({ width: 390, height: 844 })
    await byTestId(page, 'mcp-drawer-tabs-tab-tool-approvals').click()
    await expect(byTestId(page, 'mcp-tool-approvals-card')).toBeVisible({
      timeout: 10000,
    })
  })
})
