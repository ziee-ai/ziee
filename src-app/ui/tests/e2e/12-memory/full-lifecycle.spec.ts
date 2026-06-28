import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * Comprehensive memory full-lifecycle E2E (the 12-memory specs are otherwise
 * isolated single-scenario tests). Chains the whole flow against the REAL
 * backend, no mocks: admin enables memory deployment-wide → user opts into
 * retrieval → create a memory → list it → update it → set a per-conversation
 * override → delete it → confirm it's gone.
 */
test.describe('Memory — full lifecycle', () => {
  test('admin-enable → user retrieval → memory CRUD → per-conversation mode → delete', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const adminAuth = { Authorization: `Bearer ${adminToken}` }

    // 1. Admin enables memory deployment-wide.
    const enable = await page.request.put(
      `${apiURL}/api/memory/admin-settings`,
      { headers: adminAuth, data: { enabled: true } },
    )
    expect(enable.ok()).toBe(true)
    expect((await enable.json()).enabled).toBe(true)

    // 2. A regular user with memory permissions.
    const username = `mlife_${Date.now().toString(36)}`
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
        'conversations::read',
        'conversations::edit',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const auth = { Authorization: `Bearer ${userToken}` }

    // 3. User opts into retrieval (per-account setting round-trips).
    const setRetrieval = await page.request.put(
      `${apiURL}/api/memory/settings`,
      { headers: auth, data: { retrieval_enabled: true } },
    )
    expect(setRetrieval.ok()).toBe(true)
    const settings = await page.request.get(`${apiURL}/api/memory/settings`, {
      headers: auth,
    })
    expect((await settings.json()).retrieval_enabled).toBe(true)

    // 4. Create a memory.
    const createRes = await page.request.post(`${apiURL}/api/memories`, {
      headers: auth,
      data: { content: 'User prefers metric units', kind: 'fact' },
    })
    expect(createRes.status()).toBe(201)
    const memoryId: string = (await createRes.json()).id
    expect(memoryId).toBeTruthy()

    // 5. List shows it (MemoryListResponse wraps rows under `items`).
    const list = await page.request.get(`${apiURL}/api/memories`, {
      headers: auth,
    })
    const rows = (await list.json()).items as { id: string }[]
    expect(rows.some(m => m.id === memoryId)).toBe(true)

    // 6. Update its content (PATCH) and confirm the change persisted.
    const patch = await page.request.patch(
      `${apiURL}/api/memories/${memoryId}`,
      { headers: auth, data: { content: 'User prefers imperial units' } },
    )
    expect(patch.ok()).toBe(true)
    const got = await page.request.get(
      `${apiURL}/api/memories/${memoryId}`,
      { headers: auth },
    )
    expect((await got.json()).content).toContain('imperial')

    // 7. Per-conversation override: turn memory OFF for one conversation.
    const conv = await page.request.post(`${apiURL}/api/conversations`, {
      headers: auth,
      data: { title: 'mlife-conv' },
    })
    const conversationId: string = (await conv.json()).id
    const setMode = await page.request.put(
      `${apiURL}/api/conversations/${conversationId}/memory-mode`,
      { headers: auth, data: { memory_mode: 'off' } },
    )
    expect(setMode.ok()).toBe(true)
    expect((await setMode.json()).memory_mode).toBe('off')

    // 8. Delete the memory → it is gone.
    const del = await page.request.delete(
      `${apiURL}/api/memories/${memoryId}`,
      { headers: auth },
    )
    expect(del.ok()).toBe(true)
    const after = await page.request.get(
      `${apiURL}/api/memories/${memoryId}`,
      { headers: auth },
    )
    expect(after.status()).toBe(404)
  })
})
