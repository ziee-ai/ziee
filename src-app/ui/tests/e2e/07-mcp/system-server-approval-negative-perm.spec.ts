import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from '../mcp/helpers/navigation-helpers'
import { clickEditServerButton } from '../mcp/helpers/form-helpers'

/**
 * TEST-190 (A10 [negative-perm]) — the per-tool approval surface
 * (`McpToolApprovalsTab` → the "Tool approvals" tab / `mcp-tool-approvals-card`)
 * is an ADMIN-ONLY, SYSTEM-SERVER-ONLY control: it renders inside the MCP server
 * drawer ONLY in `edit-system` mode, which is reachable only from the System MCP
 * Servers admin page (`/settings/mcp-admin`, gated `mcp_servers::admin::read`).
 *
 * This spec proves an UNPERMITTED (non-admin) user sees NONE of it, walking all
 * four gating layers, with the admin as the positive control:
 *
 *   1. slot   — the `settingsAdminPages` "MCP servers" (mcp-admin) nav item is
 *               filtered out of the settings menu.
 *   2. route  — a direct hit on `/settings/mcp-admin` renders a 403 gate, never
 *               the SystemMcpServersPage.
 *   3. mode   — the non-admin can never reach `edit-system` mode (its only entry
 *               point is the admin page they cannot open), so no edit-system
 *               drawer ever mounts for them.
 *   4. tab    — the "Tool approvals" tab (`mcp-drawer-tabs-tab-tool-approvals`)
 *               and its `mcp-tool-approvals-card` never appear anywhere the user
 *               CAN navigate (incl. their own `/settings/mcp-servers` page, whose
 *               drawers only ever open in `edit` mode — the tab is `edit-system`).
 */
test.describe('System MCP tool-approvals — permission gating (negative-perm)', () => {
  test('admin sees the Tool approvals surface; a non-admin sees it at no gating layer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // ── Positive control: an admin CAN see the Tool approvals tab. ──────────
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Seed a deterministic system server so there is a card to open in
    // edit-system mode (a dead loopback URL is fine — the tab still renders;
    // its body just reports the tools probe as unreachable).
    const fixtureName = `approval-fixture-${Date.now().toString(36)}`
    const fixtureDisplay = `Approval Fixture ${Date.now().toString(36)}`
    const createRes = await page.request.post(
      `${apiURL}/api/mcp/system-servers`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
        data: {
          name: fixtureName,
          display_name: fixtureDisplay,
          description: 'Fixture system server for the tool-approvals A10 spec',
          transport_type: 'http',
          url: 'http://127.0.0.1:9/mcp',
          enabled: false,
        },
      },
    )
    expect(createRes.ok()).toBe(true)

    await goToMcpAdminPage(page, baseURL)
    await waitForMcpAdminPageLoad(page)

    // Open the fixture server's drawer (system server → edit-system mode).
    await clickEditServerButton(page, fixtureDisplay, true)
    await expect(byTestId(page, 'mcp-drawer-tabs')).toBeVisible({
      timeout: 15000,
    })
    // The edit-system-only "Tool approvals" tab is present …
    const approvalsTab = byTestId(page, 'mcp-drawer-tabs-tab-tool-approvals')
    await expect(approvalsTab).toBeVisible({ timeout: 15000 })
    // … and clicking it reveals the admin approval-override card.
    await approvalsTab.click()
    await expect(byTestId(page, 'mcp-tool-approvals-card')).toBeVisible({
      timeout: 15000,
    })

    // ── Negative: a normal non-admin user (default group only). ─────────────
    // Created via the admin API with just profile perms; the backend auto-adds
    // every new user to the default `users` group (which grants
    // `mcp_servers::read` — so they CAN see their OWN MCP page — but NOT
    // `mcp_servers::admin::read`), making this a realistic non-admin subject.
    const uname = `mcpnoadmin_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')

    // Layer 1 (slot): the settings menu renders, but the admin "MCP servers"
    // (mcp-admin) item is filtered out. The USER-facing mcp-servers item may
    // still show (they hold mcp_servers::read) — that is a different surface.
    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    await expect(
      byTestId(page, 'settings-nav-menu-item-mcp-admin'),
    ).toHaveCount(0)

    // Layer 2 (route): a direct hit on /settings/mcp-admin renders a 403 gate,
    // not the SystemMcpServersPage (its admin-only Add button never appears),
    // and the approvals surface is absent.
    await page.goto(`${baseURL}/settings/mcp-admin`)
    await expect(
      page.locator(
        '[data-testid="router-route-forbidden-result"], [data-testid="settings-forbidden-result"]',
      ),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'mcp-system-add-btn')).toHaveCount(0)
    await expect(byTestId(page, 'mcp-tool-approvals-card')).toHaveCount(0)
    await expect(
      byTestId(page, 'mcp-drawer-tabs-tab-tool-approvals'),
    ).toHaveCount(0)

    // Layers 3 + 4 (mode + tab): on the user's OWN reachable MCP page there is
    // no edit-system entry point, so the edit-system-only Tool approvals tab /
    // card never renders for this user anywhere they can go.
    await page.goto(`${baseURL}/settings/mcp-servers`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'mcp-tool-approvals-card')).toHaveCount(0)
    await expect(
      byTestId(page, 'mcp-drawer-tabs-tab-tool-approvals'),
    ).toHaveCount(0)
  })
})
