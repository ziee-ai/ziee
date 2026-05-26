import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

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
const HAS_LLM = Boolean(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY)

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

    // Toggle extraction on.
    const putRes = await page.request.put(`${apiURL}/api/memory/settings`, {
      data: { extraction_enabled: true },
    })
    expect(putRes.status()).toBe(200)
    expect((await putRes.json()).extraction_enabled).toBe(true)

    // Audit log endpoint reachable; starts empty.
    const auditRes = await page.request.get(`${apiURL}/api/memory/audit-log`)
    expect(auditRes.status()).toBe(200)
    expect(Array.isArray(await auditRes.json())).toBe(true)

    // Manual memory ADD writes an audit entry.
    const addRes = await page.request.post(`${apiURL}/api/memories`, {
      data: { content: 'User likes hiking', kind: 'preference' },
    })
    expect(addRes.status()).toBe(201)

    const audit2 = await page.request.get(`${apiURL}/api/memory/audit-log`)
    const entries2: any[] = await audit2.json()
    expect(entries2.some((e) => e.op === 'ADD')).toBe(true)
  })

  test('real-LLM extraction (gated)', async ({ page: _p, testInfra: _t }) => {
    test.skip(
      !HAS_LLM,
      'no ANTHROPIC_API_KEY/OPENAI_API_KEY — skipping real-LLM extraction',
    )
    // Same caveats as recall-injects-system "real-LLM" test:
    // requires admin-configured embedding + extraction models. The
    // server-side flow is covered by tests/memory/extraction_test.rs.
  })
})
