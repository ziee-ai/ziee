import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'
import {
  openAddServerDrawer,
  fillMcpServerForm,
  submitMcpServerForm,
  type McpServerFormData,
} from '../07-mcp/helpers/form-helpers'

// Realtime sync for the MCP module. Three entities are exercised:
//
//   1. mcp_server          (OWNER-scoped)        — a user's OWN MCP server
//      reaches the SAME user's other device, and never a different user.
//   2. mcp_server_system   (PERMISSION-scoped,   mcp_servers_admin::read) — an
//      admin's SYSTEM server reaches another admin device's system table.
//   3. user_mcp_server     (CROSS-ROLE,          mcp_servers::read)      — an
//      admin assigning a SYSTEM server to a user's group updates that user's
//      accessible-MCP view live (via the sync:user_mcp_server event).
//
// Run with --workers=1 (shared backend + DB).
//
// CRITICAL: `waitForLoadState('networkidle')` HANGS forever against the
// persistent realtime-sync SSE stream (the network is never idle). These
// specs navigate inline and wait on a stable page selector (the heading)
// instead — the same `load`-based pattern the 07-mcp nav helpers now use.

// Navigate to the user MCP servers page WITHOUT networkidle.
async function goToUserMcpPage(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-servers`)
  // The Add button only renders once the page data has loaded — a stable
  // "page is ready" signal (same as the 07-mcp nav helpers).
  await byTestId(page, 'mcp-settings-add-btn').waitFor({ state: 'visible', timeout: 30_000 })
}

// Navigate to the admin (system) MCP servers page WITHOUT networkidle.
async function goToAdminMcpPage(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/mcp-admin`)
  await byTestId(page, 'mcp-system-add-btn').waitFor({ state: 'visible', timeout: 30_000 })
}

// The server card surfaces the server's display_name (user: mcp-server-card-<id>,
// admin: mcp-system-server-card-<id>).
function getServerCard(page: import('@playwright/test').Page, displayName: string) {
  return page
    .getByTestId(/^mcp-(system-)?server-card-/)
    .filter({ hasText: displayName })
    .first()
}

test.describe('Realtime sync — MCP', () => {
  // ──────────────────────────────────────────────────────────────────────
  // Entity 1 — mcp_server (OWNER-scoped)
  // ──────────────────────────────────────────────────────────────────────
  test("a user's own MCP server reaches the owner's other device but NOT a different user", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Create the admin first (loginAsAdmin onboards on the fresh per-test
    // backend) so getAdminToken below can authenticate.
    await loginAsAdmin(page, baseURL)
    await goToUserMcpPage(page, baseURL)

    // A second, distinct user. createTestUser auto-joins the default Users
    // group, so B gets a working app shell + a live sync stream. Grant
    // mcp_servers::read so the user's accessible-MCP store actually fetches
    // (it self-gates on that permission).
    const adminToken = await getAdminToken(baseURL)
    const uniq = Date.now()
    const username = `mcp_other_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'mcp_servers::read'],
    )

    const ctxA2 = await browser.newContext() // owner (admin), device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // different user — isolation
    const pageB = await ctxB.newPage()
    try {
      // Load device A2 (owner) fully before opening device B.
      await loginAsAdmin(pageA2, baseURL)
      await goToUserMcpPage(pageA2, baseURL)

      await login(pageB, baseURL, username, password)
      await goToUserMcpPage(pageB, baseURL)

      // Owner (admin) creates an OWN (non-system) MCP server on device A.
      const serverData: McpServerFormData = {
        name: `sync-owner-${uniq}`,
        displayName: `Sync Owner ${uniq}`,
        description: 'Owner-scoped sync server',
        transportType: 'http',
        url: 'https://owner-sync.example.com/mcp',
        enabled: true,
      }
      await openAddServerDrawer(page)
      await fillMcpServerForm(page, serverData)
      await submitMcpServerForm(page, 'create')

      // Positive control: the owner's OTHER device receives it live — the
      // sync:mcp_server event makes the accessible-MCP store refetch. This is
      // also the synchronization point that makes B's absence below meaningful.
      await expect(getServerCard(pageA2, serverData.displayName)).toBeVisible({
        timeout: 15_000,
      })

      // Isolation: a different user (same delivery window) never sees the
      // owner's personal server.
      await expect(getServerCard(pageB, serverData.displayName)).not.toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })

  // ──────────────────────────────────────────────────────────────────────
  // Entity 2 — mcp_server_system (PERMISSION-scoped: mcp_servers_admin::read)
  // ──────────────────────────────────────────────────────────────────────
  test('an admin system MCP server created on device A appears on admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToAdminMcpPage(page, baseURL)

    const ctxB = await browser.newContext() // same admin, device 2
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await goToAdminMcpPage(pageB, baseURL)

      const uniq = Date.now()
      const serverData: McpServerFormData = {
        name: `sync-system-${uniq}`,
        displayName: `Sync System ${uniq}`,
        description: 'Permission-scoped sync system server',
        transportType: 'http',
        url: 'https://system-sync.example.com/mcp',
        enabled: true,
      }

      // Create the SYSTEM server on device A (isSystemServer = true).
      await openAddServerDrawer(page, true)
      await fillMcpServerForm(page, serverData)
      await submitMcpServerForm(page, 'create', true)

      // Device B's system-servers table must show it WITHOUT a manual reload —
      // the sync:mcp_server_system event makes the system store refetch.
      await expect(getServerCard(pageB, serverData.displayName)).toBeVisible({
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  // ──────────────────────────────────────────────────────────────────────
  // Entity 3 — user_mcp_server (CROSS-ROLE: mcp_servers::read)
  //
  // A bare system server is NOT auto-assigned to any group, so it does NOT
  // appear in a user's listAccessible until an admin assigns it to a group the
  // user belongs to (verified in server/src/modules/mcp/repository.rs —
  // list_accessible joins user_group_mcp_servers). The assign endpoint
  // (POST /api/mcp/system-servers/{id}/groups) publishes sync:user_mcp_server
  // to every mcp_servers::read holder, which is the live trigger we assert on.
  // ──────────────────────────────────────────────────────────────────────
  test("assigning a system MCP server to a user's group updates that user's accessible view live", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Admin first (onboards the fresh backend), then create the regular user.
    await loginAsAdmin(page, baseURL)
    await goToAdminMcpPage(page, baseURL)

    const adminToken = await getAdminToken(baseURL)
    const uniq = Date.now()
    const username = `mcp_xrole_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'mcp_servers::read'],
    )

    // Resolve the default Users group (createTestUser auto-joined the new user
    // into it). list_groups returns { groups: [{ id, is_default, ... }] }.
    const groupsRes = await fetch(`${baseURL}/api/groups?page=1&per_page=100`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    if (!groupsRes.ok) {
      throw new Error(`list groups failed: ${groupsRes.status} - ${await groupsRes.text()}`)
    }
    const { groups } = await groupsRes.json()
    const defaultGroup = (groups as Array<{ id: string; is_default: boolean }>).find(
      g => g.is_default,
    )
    if (!defaultGroup) {
      throw new Error('No default (is_default) user group found to assign the system server to')
    }

    const ctxB = await browser.newContext() // the regular user — cross-role receiver
    const pageB = await ctxB.newPage()
    try {
      // Load the user device fully before mutating from the admin device.
      await login(pageB, baseURL, username, password)
      await goToUserMcpPage(pageB, baseURL)

      // Admin creates the SYSTEM server in the UI on device A.
      const serverData: McpServerFormData = {
        name: `sync-xrole-${uniq}`,
        displayName: `Sync XRole ${uniq}`,
        description: 'Cross-role sync system server',
        transportType: 'http',
        url: 'https://xrole-sync.example.com/mcp',
        enabled: true,
      }
      await openAddServerDrawer(page, true)
      await fillMcpServerForm(page, serverData)
      await submitMcpServerForm(page, 'create', true)

      // Pre-condition: the user does NOT yet see the server — it isn't assigned
      // to their group, so listAccessible excludes it.
      await expect(getServerCard(pageB, serverData.displayName)).not.toBeVisible()

      // Look up the freshly-created system server's id via the admin API.
      const sysRes = await fetch(
        `${baseURL}/api/mcp/system-servers?page=1&per_page=200`,
        { headers: { Authorization: `Bearer ${adminToken}` } },
      )
      if (!sysRes.ok) {
        throw new Error(`list system servers failed: ${sysRes.status} - ${await sysRes.text()}`)
      }
      const sysBody = await sysRes.json()
      const sysList: Array<{ id: string; name: string }> =
        sysBody.servers ?? sysBody.system_servers ?? sysBody
      const created = sysList.find(s => s.name === serverData.name)
      if (!created) {
        throw new Error(`Could not find created system server "${serverData.name}" in admin list`)
      }

      // Assign the system server to the user's default group. This publishes
      // sync:user_mcp_server to mcp_servers::read holders → the user's device.
      const assignRes = await fetch(
        `${baseURL}/api/mcp/system-servers/${created.id}/groups`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          body: JSON.stringify({ group_ids: [defaultGroup.id] }),
        },
      )
      if (!assignRes.ok) {
        throw new Error(`assign-to-group failed: ${assignRes.status} - ${await assignRes.text()}`)
      }

      // The user's accessible-MCP view updates live (no manual reload) — the
      // sync:user_mcp_server event makes listAccessible refetch and the now-
      // group-visible system server appears.
      await expect(getServerCard(pageB, serverData.displayName)).toBeVisible({
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })
})
