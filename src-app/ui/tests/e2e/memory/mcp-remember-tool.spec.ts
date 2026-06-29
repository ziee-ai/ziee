import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MCP `remember` tool roundtrip (Plan §9 Phase 4).
 *
 * Drives the JSON-RPC endpoint directly. The full mcp/client roundtrip
 * test that the plan also calls for lives in the backend integration
 * suite (tests/memory_mcp/mod.rs::test_remember_then_forget_roundtrip).
 */

test.describe('Memory MCP — remember/forget tools', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('remember tool persists; appears on Memories page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `mcp_e2e_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::read',
        'memory::write',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)

    // Call the remember tool via JSON-RPC.
    const res = await page.request.post(`${apiURL}/api/memories/mcp`, {
      headers: { Authorization: `Bearer ${userToken}` },
      data: {
        jsonrpc: '2.0',
        id: 1,
        method: 'tools/call',
        params: {
          name: 'remember',
          arguments: { content: 'User uses Linux on a ThinkPad' },
        },
      },
    })
    expect(res.ok()).toBe(true)
    const body = await res.json()
    expect(body.result?.structuredContent?.memory_id).toBeTruthy()

    // Now visit /settings/memory and confirm it shows up. Memories
    // render as plain divs with `data-memory-id`, not table cells.
    // Scope to the My memories section's wrapper, then assert one of
    // the data-memory-id divs contains the snapshot text. The audit
    // log section renders the same string too, so the data-memory-id
    // selector disambiguates without needing card-scoping.
    await page.goto(`${baseURL}/settings/memory`)
    await expect(
      page
        .locator('[data-memory-id]')
        .filter({ hasText: 'User uses Linux on a ThinkPad' })
        .first(),
    ).toBeVisible({ timeout: 5000 })
  })
})
