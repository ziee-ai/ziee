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
 * Exercises the memory-owned endpoints
 * `GET`/`PUT /api/conversations/{id}/memory-mode` that replaced the
 * chat-side `PATCH /api/conversations/{id}` body field after
 * migration 76 moved the column into `conversation_memory_settings`.
 * The composer pill (MemoryStatusPill.tsx) is one consumer; this
 * test drives the API directly so the test isn't coupled to the
 * pill's placement in the chat layout.
 */

test.describe('Memory — per-conversation override', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('memory_mode round-trips through PUT /memory-mode', async ({
    page,
    testInfra,
  }) => {
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

    // Create a conversation (chat-owned endpoint).
    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'pcm-test' },
    })
    expect(created.ok()).toBe(true)
    const conv = await created.json()

    // Default is 'inherit' (no row in conversation_memory_settings).
    const initial = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/memory-mode`,
      { headers: authHeader },
    )
    expect(initial.ok()).toBe(true)
    expect(((await initial.json()) as any).memory_mode).toBe('inherit')

    // Flip to 'off' via the memory-owned PUT.
    const off = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/memory-mode`,
      {
        headers: authHeader,
        data: { memory_mode: 'off' },
      },
    )
    expect(off.ok()).toBe(true)
    expect(((await off.json()) as any).memory_mode).toBe('off')

    // GET round-trip sees the new value.
    const after = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/memory-mode`,
      { headers: authHeader },
    )
    expect(((await after.json()) as any).memory_mode).toBe('off')

    // Invalid value rejected with 400.
    const bad = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/memory-mode`,
      {
        headers: authHeader,
        data: { memory_mode: 'maybe' },
      },
    )
    expect(bad.status()).toBe(400)
  })
})
