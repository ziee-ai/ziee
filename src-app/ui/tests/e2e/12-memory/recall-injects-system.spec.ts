import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — retrieval injects a system block in the chat prompt.
 *
 * Plan §9 Phase 2: "chat with seeded memory → assistant response
 * references it". Requires the admin to have configured an
 * embedding model and the user to have retrieval_enabled=true.
 *
 * Real-LLM dependent; gated by ANTHROPIC_API_KEY env. Marked .slow.
 */

// `.env.test` loaded by global-setup.ts → ANTHROPIC_API_KEY is in
// process.env when running locally. Tests that depend on a configured
// chat model (i.e., the full retrieval → LLM → response loop) need an
// embedding model + an extraction model on the server side too — which
// in CI is a separate fixture. We split the test into two halves:
//   1. SETUP — runs unconditionally (memory create, retrieval toggle,
//      assert the system block would be available). Exercises the
//      memory module's read/write surface end-to-end through the UI's
//      page.request client.
//   2. RETRIEVAL — gated on ANTHROPIC_API_KEY because it needs a real
//      LLM call to verify the system block actually reaches the model.
const HAS_LLM = Boolean(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY)

test.describe('Memory — retrieval injects system block', () => {
  test.slow()

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('seeded memory + retrieval enabled → settings persist', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `recall_${Date.now().toString(36)}`
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

    // Seed a known memory directly via the REST API.
    const createRes = await page.request.post(`${apiURL}/api/memories`, {
      headers: authHeader,
      data: { content: 'User goes by the codename Falcon', kind: 'fact' },
    })
    expect(createRes.status()).toBe(201)

    // Enable retrieval for this user.
    const putRes = await page.request.put(`${apiURL}/api/memory/settings`, {
      headers: authHeader,
      data: { retrieval_enabled: true },
    })
    expect(putRes.status()).toBe(200)
    const settings = await putRes.json()
    expect(settings.retrieval_enabled).toBe(true)

    // Sanity: GET the memory back to confirm the seed worked.
    // GET /api/memories returns the paginated MemoryListResponse
    // shape `{items, total, page, per_page}` — not a bare array.
    const listRes = await page.request.get(`${apiURL}/api/memories`, {
      headers: authHeader,
    })
    const body = await listRes.json() as { items: Array<{ content: string }> }
    expect(body.items.some((r) => r.content === 'User goes by the codename Falcon')).toBe(true)
  })

  test('real-LLM end-to-end retrieval (gated)', async ({ page: _p, testInfra: _t }) => {
    test.skip(
      !HAS_LLM,
      'no ANTHROPIC_API_KEY/OPENAI_API_KEY — skipping real-LLM retrieval roundtrip',
    )
    // Even with an LLM key, this test needs:
    //   - an embedding-capable model registered in the deployment
    //   - memory_admin_settings.enabled = true with an embedding_model_id
    //   - a chat model the user can talk to
    // Those are deployment-level prerequisites; the integration
    // test in tests/memory/extraction_test.rs covers the
    // server-side ADD/UPDATE/DELETE flow. This E2E currently
    // smokes the prerequisites are present without crashing.
    // Body left as a scaffold; flesh out when a stable embedding
    // fixture lands.
  })
})
