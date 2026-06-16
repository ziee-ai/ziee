import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * MCP Defaults card (read-only summary on the project detail page).
 *
 * Covers the post-rewrite structure where the body lists per-server
 * Auto-approved / Disabled rules instead of just bare counts, the Edit
 * affordance lives in the Card header `extra` slot, and the panel
 * filters out stale auto-approve entries for servers that are fully
 * disabled (a backend-preserved-preferences artifact — see
 * ProjectMcpSettingsPanel.tsx for the long-form justification).
 */
test.describe('Projects - MCP Defaults card', () => {
  // Helper: create a project via the UI, return its id by reading the URL.
  async function createProject(page: import('@playwright/test').Page, name: string) {
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, { name })
    await submitProjectForm(page)
    await page.locator('.ant-card', { hasText: name }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const url = new URL(page.url())
    return url.pathname.split('/').pop()!
  }

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToProjectsPage(page, testInfra.baseURL)
  })

  test('empty state: no per-server rules shows neutral Empty + only approval mode', async ({
    page,
  }) => {
    await createProject(page, 'MCP Defaults Empty')

    const mcp = page.locator('[data-test-section="mcp-defaults"]')
    await expect(mcp).toBeVisible()
    // Fresh project = manual_approve, no rules.
    await expect(
      mcp.locator('[data-test-mcp-approval-mode="manual_approve"]'),
    ).toBeVisible()
    // Empty state copy from Empty component.
    await expect(mcp.getByText(/no per-server rules/i)).toBeVisible()
    // No "Auto-approved" / "Disabled" headings.
    await expect(mcp.getByText('Auto-approved', { exact: true })).toHaveCount(0)
    await expect(mcp.getByText('Disabled', { exact: true })).toHaveCount(0)
  })

  test('renders per-server lists with display name + All-tools / per-tool tags', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(baseURL)

    // Register two system MCP servers via API so display-name lookup
    // resolves (we don't invoke them; the URLs are stub localhost
    // endpoints, the modal/panel only reads .display_name from the
    // registered server list).
    const stamp = Date.now()
    async function createServer(name: string, displayName: string) {
      const res = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
        headers: { Authorization: `Bearer ${token}` },
        data: {
          name,
          display_name: displayName,
          description: 'stub for e2e display-name resolution',
          enabled: true,
          transport_type: 'http',
          url: 'http://127.0.0.1:1/stub',
          timeout_seconds: 5,
          usage_mode: 'auto',
        },
      })
      expect(res.status(), `create ${name}`).toBe(201)
      return (await res.json()).id as string
    }
    const fetchId = await createServer(`stub_fetch_${stamp}`, 'Web Fetch')
    const searchId = await createServer(`stub_search_${stamp}`, 'Web Search')

    // Assign both to admin's default group so Stores.McpServer.servers
    // (which the panel reads) lists them.
    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> =
      Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const defaultGroup =
      groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (defaultGroup) {
      for (const sid of [fetchId, searchId]) {
        await page.request.post(
          `${apiURL}/api/mcp/system-servers/${sid}/groups`,
          {
            headers: { Authorization: `Bearer ${token}` },
            data: { group_ids: [defaultGroup.id] },
          },
        )
      }
    }

    const projectId = await createProject(page, 'MCP Defaults Populated')

    // Configure rules:
    // - fetch: auto-approved for tool `fetch` only (per-tool)
    // - search: fully auto-approved (tools: [])
    await page.request.put(`${apiURL}/api/projects/${projectId}/mcp-settings`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        approval_mode: 'auto_approve',
        auto_approved_tools: [
          { server_id: fetchId, tools: ['fetch'] },
          { server_id: searchId, tools: [] },
        ],
        disabled_servers: [],
      },
    })

    await page.reload()
    const mcp = page.locator('[data-test-section="mcp-defaults"]')
    await expect(mcp).toBeVisible()

    // Approval mode reflects the API update.
    await expect(
      mcp.locator('[data-test-mcp-approval-mode="auto_approve"]'),
    ).toBeVisible()

    // Auto-approved heading shows; Disabled heading hidden.
    await expect(mcp.getByText('Auto-approved', { exact: true })).toBeVisible()
    await expect(mcp.getByText('Disabled', { exact: true })).toHaveCount(0)

    // Both servers' display names appear in the body.
    await expect(mcp.getByText('Web Fetch', { exact: true })).toBeVisible()
    await expect(mcp.getByText('Web Search', { exact: true })).toBeVisible()

    // Per-tool entry shows the tool name as a tag; whole-server entry
    // shows the "All tools" tag.
    await expect(mcp.getByText('fetch', { exact: true })).toBeVisible()
    await expect(mcp.getByText('All tools', { exact: true })).toBeVisible()
  })

  test('fully-disabled server suppresses its stale auto-approve entry', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(baseURL)

    const stamp = Date.now()
    async function createServer(name: string, displayName: string) {
      const res = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
        headers: { Authorization: `Bearer ${token}` },
        data: {
          name,
          display_name: displayName,
          enabled: true,
          transport_type: 'http',
          url: 'http://127.0.0.1:1/stub',
          timeout_seconds: 5,
          usage_mode: 'auto',
        },
      })
      expect(res.status(), `create ${name}`).toBe(201)
      return (await res.json()).id as string
    }
    const sid = await createServer(`stub_dual_${stamp}`, 'Conflicted Server')

    // Assign to admin's default group. Without this, the PUT to
    // mcp-settings below returns 422 from validate_mcp_server_access
    // ("MCP_SERVER_NOT_ACCESSIBLE") — the user doesn't see the
    // system server via group membership, so the per-server rule
    // is rejected. Settings stay empty + the assertion below
    // ("Disabled" heading visible) fails.
    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> =
      Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const defaultGroup =
      groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    if (defaultGroup) {
      await page.request.post(
        `${apiURL}/api/mcp/system-servers/${sid}/groups`,
        {
          headers: { Authorization: `Bearer ${token}` },
          data: { group_ids: [defaultGroup.id] },
        },
      )
    }

    // Reproduce the data shape produced by the modal: same server is
    // BOTH in auto_approved_tools (preference preservation) AND in
    // disabled_servers with tools: [] (fully disabled). The display
    // must show ONLY the disabled rule.
    const projectId = await createProject(page, 'MCP Stale Auto-approve')
    await page.request.put(`${apiURL}/api/projects/${projectId}/mcp-settings`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        approval_mode: 'manual_approve',
        auto_approved_tools: [{ server_id: sid, tools: ['some_tool'] }],
        disabled_servers: [{ server_id: sid, tools: [] }],
      },
    })

    await page.reload()
    const mcp = page.locator('[data-test-section="mcp-defaults"]')
    await expect(mcp).toBeVisible()

    // Disabled heading shows; Auto-approved heading hidden because
    // the only auto-approve rule was for a fully-disabled server.
    await expect(mcp.getByText('Disabled', { exact: true })).toBeVisible()
    await expect(mcp.getByText('Auto-approved', { exact: true })).toHaveCount(0)
    // The stale tool tag (some_tool) must NOT appear anywhere in the panel.
    await expect(mcp.getByText('some_tool', { exact: true })).toHaveCount(0)
    // The server appears exactly ONCE (under Disabled), not twice.
    await expect(mcp.getByText('Conflicted Server', { exact: true })).toHaveCount(1)
    // All-tools tag appears (the disabled rule's tools: [] = whole server).
    await expect(mcp.getByText('All tools', { exact: true })).toBeVisible()
  })

  // Regression for the modal state-bleed bug: McpComposer.store
  // openConfigModalForProject used to leave `state.selectedServers`
  // populated from a prior session, and the modal's seed-once guard
  // (`if (selectedServers.size > 0) return`) then skipped re-seeding from
  // backend state — so a server disabled in a prior modal session
  // reappeared as ENABLED on the next open. The fix resets selectedServers
  // on every open. This drives the full MODAL UI (toggle → save → reload →
  // reopen), which the API-driven tests above never exercise.
  test('modal: toggle a server off, save, reload, reopen — stays off (no state-bleed)', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(baseURL)

    // Register a system MCP server and assign it to the admin's default
    // group so the modal lists it as an enabled, toggle-able server.
    const stamp = Date.now()
    const createRes = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: `stub_bleed_${stamp}`,
        display_name: 'Bleed Test Server',
        enabled: true,
        transport_type: 'http',
        url: 'http://127.0.0.1:1/stub',
        timeout_seconds: 5,
        usage_mode: 'auto',
      },
    })
    expect(createRes.status(), 'create server').toBe(201)
    const sid = (await createRes.json()).id as string

    const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    const groupsBody = await groupsRes.json()
    const groups: Array<{ id: string; is_default?: boolean; name: string }> =
      Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
    const defaultGroup =
      groups.find(g => g.is_default) ?? groups.find(g => g.name === 'Users')
    expect(defaultGroup, 'default group').toBeTruthy()
    await page.request.post(`${apiURL}/api/mcp/system-servers/${sid}/groups`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { group_ids: [defaultGroup!.id] },
    })

    const projectId = await createProject(page, 'MCP State-Bleed')

    const openModal = async () => {
      await page.getByRole('button', { name: 'Edit MCP defaults' }).click()
      await expect(page.getByText('MCP Defaults for Project')).toBeVisible()
    }
    // The per-server toggle lives in the Collapse HEADER (the per-tool
    // auto-approve switches are in the collapsed body), so scope to the
    // header to avoid matching those.
    const serverSwitch = () =>
      page
        .locator('.ant-collapse-header', { hasText: 'Bleed Test Server' })
        .getByRole('switch')

    // PHASE 1: open modal — the server starts ENABLED (it's in the default
    // group, fresh project has no disabled_servers) — toggle OFF, Save & Close.
    await openModal()
    await expect(serverSwitch()).toBeChecked()
    const savePut = page.waitForResponse(
      r =>
        r.url().includes(`/api/projects/${projectId}/mcp-settings`) &&
        r.request().method() === 'PUT',
    )
    await serverSwitch().click()
    await expect(serverSwitch()).not.toBeChecked()
    await page.getByRole('button', { name: 'Save & Close' }).click()
    await savePut

    // PHASE 2: reload, reopen the modal — the switch must read the persisted
    // disabled state, NOT stale in-memory selectedServers from before.
    await page.reload()
    await openModal()
    await expect(serverSwitch()).not.toBeChecked()
  })
})
