import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — assistant core memory editor (Plan §9 Phase 6).
 *
 * Exercises the CRUD on /api/assistants/{id}/core-memory. The actual
 * block-injection behavior in the chat path is tested in the backend
 * integration suite (memory module retriever tests).
 */

test.describe('Memory — assistant core memory blocks', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('block_label validation rejects bad slugs', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `core_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::core::read',
        'memory::core::write',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)

    // Random UUID — the FK to assistants(id) will reject, but the
    // 400 we expect for the bad slug fires BEFORE the FK check
    // because validation happens in the handler.
    const assistantId = '00000000-0000-0000-0000-000000000001'
    const res = await page.request.put(`${apiURL}/api/assistants/core-memory`, {
      headers: { Authorization: `Bearer ${userToken}` },
      data: {
        assistant_id: assistantId,
        block_label: 'NOT A VALID SLUG',
        content: 'hi',
        char_limit: 100,
      },
    })
    expect(res.status()).toBe(400)
  })
})
