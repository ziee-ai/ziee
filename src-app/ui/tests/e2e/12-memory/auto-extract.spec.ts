import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../09-chat/helpers/chat-helpers'

/**
 * E2E — auto-extraction toggle path (Plan §9 Phase 3).
 *
 * Two halves:
 *   1. SETUP — runs unconditionally. Toggles extraction_enabled,
 *      verifies audit-log endpoint is reachable, asserts an ADD
 *      audit entry on manual memory creation. Exercises the full
 *      REST path the auto-extractor itself uses.
 *   2. REAL-LLM — gated on ANTHROPIC_API_KEY. Full chat-then-extract
 *      roundtrip requires admin-configured embedding + extraction
 *      models (deployment fixtures). The server-side ADD/UPDATE/DELETE
 *      pipeline is covered by tests/memory/extraction_test.rs.
 */

test.describe('Memory — auto-extraction', () => {
  test.slow()

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('enable extraction → settings persist; audit log reachable', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `extract_${Date.now().toString(36)}`
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
    const authHeader = { Authorization: `Bearer ${userToken}` }

    // Toggle extraction on.
    const putRes = await page.request.put(`${apiURL}/api/memory/settings`, {
      headers: authHeader,
      data: { extraction_enabled: true },
    })
    expect(putRes.status()).toBe(200)
    expect((await putRes.json()).extraction_enabled).toBe(true)

    // Audit log endpoint reachable; starts empty.
    const auditRes = await page.request.get(
      `${apiURL}/api/memory/audit-log`,
      { headers: authHeader },
    )
    expect(auditRes.status()).toBe(200)
    expect(Array.isArray(await auditRes.json())).toBe(true)

    // Manual memory ADD writes an audit entry.
    const addRes = await page.request.post(`${apiURL}/api/memories`, {
      headers: authHeader,
      data: { content: 'User likes hiking', kind: 'preference' },
    })
    expect(addRes.status()).toBe(201)

    const audit2 = await page.request.get(`${apiURL}/api/memory/audit-log`, {
      headers: authHeader,
    })
    const entries2: any[] = await audit2.json()
    expect(entries2.some((e) => e.op === 'ADD')).toBe(true)
  })

  // audit id d38639e14c2e6ca7 — auto-extraction THROUGH THE CHAT UI was untested
  // (this test was previously an empty scaffold). Real-LLM flow: configure an
  // Anthropic chat model + point the extraction model at it, enable memory
  // (FTS-only so no embedding model is needed) + per-user extraction, send a
  // chat message stating a memorable fact through the real chat UI, then poll
  // the audit log for the auto-extracted ADD entry. Gated on ANTHROPIC_API_KEY.
  test('real-LLM auto-extraction through the chat UI', async ({ page, testInfra }) => {
    test.skip(
      !process.env.ANTHROPIC_API_KEY,
      'no ANTHROPIC_API_KEY — skipping real-LLM chat auto-extraction',
    )
    test.slow()
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    // Anthropic chat model (real key), assigned to Administrators so the admin
    // can chat with it.
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    const authHeader = { Authorization: `Bearer ${adminToken}` }

    // Enable memory deployment-wide, FTS-only (no embedding model needed), and
    // point the extraction model at the chat model.
    const adminPut = await page.request.put(`${apiURL}/api/memory/admin-settings`, {
      headers: authHeader,
      data: {
        enabled: true,
        fts_enabled: true,
        semantic_enabled: false,
        default_extraction_model_id: modelId,
      },
    })
    expect(adminPut.status()).toBe(200)

    // Enable per-user extraction for the admin.
    const userPut = await page.request.put(`${apiURL}/api/memory/settings`, {
      headers: authHeader,
      data: { extraction_enabled: true },
    })
    expect(userPut.status()).toBe(200)

    // Send a message stating a clear, memorable fact through the chat UI.
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Please remember this about me: my favorite programming language is Rust and I live in Oslo.',
      false,
    )

    // The assistant reply triggers the after_llm_call extraction hook (a
    // background task). Poll the audit log for the auto-extracted ADD.
    await expect
      .poll(
        async () => {
          const res = await page.request.get(`${apiURL}/api/memory/audit-log`, {
            headers: authHeader,
          })
          if (res.status() !== 200) return 0
          const entries = (await res.json()) as Array<{ op: string }>
          return entries.filter(e => e.op === 'ADD').length
        },
        { timeout: 90_000, intervals: [2000, 3000, 5000] },
      )
      .toBeGreaterThan(0)
  })
})
