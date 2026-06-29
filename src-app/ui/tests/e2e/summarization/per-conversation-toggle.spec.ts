import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — per-conversation summarization toggle (migration 91).
 *
 * Mirrors `memory/per-conversation-toggle.spec.ts`. Exercises the
 * summarization-owned endpoints
 * `GET`/`PUT /api/conversations/{id}/summarization-mode` that replace
 * what would otherwise live in memory's per-conversation table. The
 * composer pill (`SummarizationStatusPill.tsx`) is the in-app
 * consumer; this test drives the API directly so it isn't coupled to
 * the pill's placement.
 */

test.describe('Summarization — per-conversation override', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('summarization_mode round-trips through PUT /summarization-mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `pcs_${Date.now().toString(36)}`
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
      data: { title: 'pcs-test' },
    })
    expect(created.ok()).toBe(true)
    const conv = await created.json()

    // Default is 'inherit' (no row in conversation_summarization_settings).
    const initial = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader },
    )
    expect(initial.ok()).toBe(true)
    expect(((await initial.json()) as any).summarization_mode).toBe('inherit')

    // Flip to 'off' via the summarization-owned PUT.
    const off = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      {
        headers: authHeader,
        data: { summarization_mode: 'off' },
      },
    )
    expect(off.ok()).toBe(true)
    expect(((await off.json()) as any).summarization_mode).toBe('off')

    // GET round-trip sees the new value.
    const after = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader },
    )
    expect(((await after.json()) as any).summarization_mode).toBe('off')

    // off → on round-trip.
    const on = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader, data: { summarization_mode: 'on' } },
    )
    expect(on.ok()).toBe(true)
    expect(((await on.json()) as any).summarization_mode).toBe('on')
    const afterOn = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader },
    )
    expect(((await afterOn.json()) as any).summarization_mode).toBe('on')

    // on → inherit clears the override row server-side (set to default
    // deletes; the next GET should see the implicit `inherit` again).
    const inherit = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader, data: { summarization_mode: 'inherit' } },
    )
    expect(inherit.ok()).toBe(true)
    expect(((await inherit.json()) as any).summarization_mode).toBe('inherit')
    const afterInherit = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader },
    )
    expect(((await afterInherit.json()) as any).summarization_mode).toBe(
      'inherit',
    )

    // Invalid value rejected with 400.
    const bad = await page.request.put(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      {
        headers: authHeader,
        data: { summarization_mode: 'maybe' },
      },
    )
    expect(bad.status()).toBe(400)
  })

  test('GET /summary returns null when no summary row exists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `pcs_null_${Date.now().toString(36)}`
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

    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'pcs-null-test' },
    })
    const conv = await created.json()

    const summary = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summary`,
      { headers: authHeader },
    )
    expect(summary.ok()).toBe(true)
    expect(await summary.json()).toBeNull()
  })
})
