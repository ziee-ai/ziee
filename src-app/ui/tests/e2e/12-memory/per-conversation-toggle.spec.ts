import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — per-conversation memory toggle (Plan §9 Phase 5).
 *
 * Exercises PATCH /api/conversations/{id} with `memory_mode`.
 * The composer pill (ConversationMemoryToggle.tsx) is one consumer;
 * this test drives the API directly so the test isn't coupled to
 * the pill's placement in the chat layout.
 */

test.describe('Memory — per-conversation override', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('memory_mode round-trips through PATCH', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `pcm_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'conversations::read',
        'conversations::edit',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const authHeader = { Authorization: `Bearer ${userToken}` }

    // Create a conversation.
    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'pcm-test' },
    })
    expect(created.ok()).toBe(true)
    const conv = await created.json()

    // Default is 'inherit'.
    const initial = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}`,
      { headers: authHeader },
    )
    expect(((await initial.json()) as any).memory_mode).toBe('inherit')

    // Flip to 'off'.
    const off = await page.request.put(`${apiURL}/api/conversations/${conv.id}`, {
      headers: authHeader,
      data: { memory_mode: 'off' },
    })
    expect(off.ok()).toBe(true)
    expect(((await off.json()) as any).memory_mode).toBe('off')

    // Invalid value rejected.
    const bad = await page.request.put(`${apiURL}/api/conversations/${conv.id}`, {
      headers: authHeader,
      data: { memory_mode: 'maybe' },
    })
    expect(bad.status()).toBe(400)
  })
})
