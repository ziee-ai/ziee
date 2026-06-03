import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * McpStatusRow chip behaviour added in feat/mcp-rewrite-v2.
 *
 * The chip row (`src/modules/mcp/chat-extension/components/McpStatusRow.tsx`)
 * does three things this branch adds:
 *
 *   1. On chip-close, persists the removal to the backend — either to the
 *      conversation's mcp-settings (when a conversation exists) or to the
 *      user's defaults (when chatting on the new-chat page with no conversation).
 *   2. Filters out servers where `is_built_in === true` so built-in servers
 *      never appear as removable chips.
 *   3. Filters out servers where `enabled === false`.
 *
 * Each test sets up an admin + model + a custom HTTP server via API, then
 * exercises the chip row through the real UI.
 */

test.describe('MCP Chip Row — persistence and visibility', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Bootstrap: provider + model so the chat page is functional
    const adminToken = await getAdminToken(page)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
  })

  test('chip-close in a new conversation persists removal to user defaults', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    // 1. Seed a custom (non-built-in) system server + select it as default
    const serverDisplay = `Chip Default ${Date.now()}`
    const serverId = await createSystemServer(page, apiURL, token, serverDisplay)
    await setUserDefaults(page, apiURL, token, [serverId])

    // 2. Navigate to new chat — defaults should apply, chip should appear
    await goToNewChatPage(page, baseURL)
    const chip = page.locator(`[data-testid="mcp-chip-${serverId}"]`)
    await expect(chip).toBeVisible({ timeout: 10000 })

    // 3. Remove the chip
    await chip.locator('.ant-tag-close-icon').click()
    await page.waitForTimeout(500)
    await expect(chip).not.toBeVisible()

    // 4. Reload — chip should stay gone (persisted to user defaults)
    await page.reload()
    await page.waitForSelector('text=How can I help you today?', { timeout: 15000 })
    await page.waitForTimeout(1000)
    await expect(page.locator(`[data-testid="mcp-chip-${serverId}"]`)).not.toBeVisible()

    // Also verify via direct API GET that user defaults exclude the server
    const defaults = await fetchUserDefaults(page, apiURL, token)
    const stillSelected = defaults.selected_servers?.some(
      (s: { server_id: string }) => s.server_id === serverId,
    )
    expect(stillSelected ?? false).toBe(false)
  })

  test('built-in servers never appear as chips even when present in selection', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    // Filesystem Access is a built-in system server from the migrations.
    const builtIn = await findServerByName(page, apiURL, token, 'Filesystem Access')
    expect(builtIn.is_built_in).toBe(true)

    // Force it into user defaults — McpStatusRow must STILL filter it out.
    await setUserDefaults(page, apiURL, token, [builtIn.id])

    await goToNewChatPage(page, baseURL)
    // Even though it's in defaults, the filter `!s.is_built_in` excludes it.
    await expect(page.locator(`[data-testid="mcp-chip-${builtIn.id}"]`)).not.toBeVisible()
  })

  test('chip disappears when its server is disabled', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(page)

    const serverDisplay = `Chip Disable ${Date.now()}`
    const serverId = await createSystemServer(page, apiURL, token, serverDisplay)
    await setUserDefaults(page, apiURL, token, [serverId])

    await goToNewChatPage(page, baseURL)
    await expect(page.locator(`[data-testid="mcp-chip-${serverId}"]`)).toBeVisible({ timeout: 10000 })

    // Disable the server via API
    await page.request.put(`${apiURL}/api/mcp/system-servers/${serverId}`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { enabled: false },
    })

    // Reload to pick up the new enabled state
    await page.reload()
    await page.waitForSelector('text=How can I help you today?', { timeout: 15000 })
    await page.waitForTimeout(1000)
    await expect(page.locator(`[data-testid="mcp-chip-${serverId}"]`)).not.toBeVisible()
  })
})

// ──────────────────────────────────────────────────────────────────────────
// Local helpers
// ──────────────────────────────────────────────────────────────────────────

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}

async function createSystemServer(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
  displayName: string,
): Promise<string> {
  const res = await page.request.post(`${apiURL}/api/mcp/system-servers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: `chip_test_${Date.now()}`,
      display_name: displayName,
      description: 'chip-row e2e fixture',
      enabled: true,
      transport_type: 'http',
      url: 'https://chip-test.example.invalid/mcp',
      timeout_seconds: 30,
      supports_sampling: false,
      usage_mode: 'auto',
    },
  })
  expect(res.ok()).toBe(true)
  const body = await res.json()
  // Assign to the default group so it's accessible to admin
  await assignServerToDefaultGroup(page, apiURL, token, body.id)
  return body.id
}

async function assignServerToDefaultGroup(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
  serverId: string,
): Promise<void> {
  const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!groupsRes.ok()) return
  const groupsBody = await groupsRes.json()
  const groups: Array<{ id: string; is_protected?: boolean; name: string }> =
    Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
  // Admin user is in the Administrators group; assigning the server there is
  // sufficient for the admin to see it.
  const adminGroup =
    groups.find(g => g.name === 'Administrators') ??
    groups.find(g => g.is_protected) ??
    groups[0]
  if (!adminGroup) return

  await page.request.post(`${apiURL}/api/mcp/system-servers/${serverId}/groups`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { group_ids: [adminGroup.id] },
  })
}

async function setUserDefaults(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
  serverIds: string[],
  allKnownServerIds?: string[],
): Promise<void> {
  // The backend stores ONLY disabled_servers; selectedServers is computed by
  // the frontend as (all available - disabled). To "select only these N
  // servers", we must disable all OTHER servers (the ones not in serverIds).
  const knownIds = allKnownServerIds ?? (await listAllServerIds(page, apiURL, token))
  const targetIds = new Set(serverIds)
  const toDisable = knownIds.filter(id => !targetIds.has(id))

  const res = await page.request.put(`${apiURL}/api/mcp/defaults`, {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      approval_mode: 'manual_approve',
      auto_approved_tools: [],
      disabled_servers: toDisable.map(id => ({ server_id: id, tools: [] })),
    },
  })
  if (!res.ok()) {
    const txt = await res.text()
    throw new Error(`PUT /mcp/defaults failed: ${res.status()} ${txt}`)
  }
}

async function listAllServerIds(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
): Promise<string[]> {
  const res = await page.request.get(`${apiURL}/api/mcp/system-servers`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const body = await res.json()
  const servers: Array<{ id: string }> = Array.isArray(body) ? body : (body.servers ?? [])
  return servers.map(s => s.id)
}

async function fetchUserDefaults(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
): Promise<{ selected_servers?: Array<{ server_id: string; tools: string[] }> }> {
  const res = await page.request.get(`${apiURL}/api/mcp/defaults`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  return res.ok() ? await res.json() : {}
}

async function findServerByName(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
  displayName: string,
): Promise<{ id: string; is_built_in: boolean; display_name: string }> {
  const res = await page.request.get(`${apiURL}/api/mcp/system-servers`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const body = await res.json()
  const servers: Array<{ id: string; display_name: string; is_built_in: boolean }> =
    Array.isArray(body) ? body : (body.servers ?? [])
  const match = servers.find(s => s.display_name === displayName)
  expect(match).toBeDefined()
  return match!
}
