import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
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

    // Call the remember tool via JSON-RPC.
    const res = await page.request.post(`${apiURL}/api/memories/mcp`, {
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

    // Now visit /settings/memory and confirm it shows up.
    await page.goto(`${baseURL}/settings/memory`)
    await expect(
      page.getByText('User uses Linux on a ThinkPad'),
    ).toBeVisible({ timeout: 5000 })
  })
})
