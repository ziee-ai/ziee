import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — admin picks an embedding model on the Memory admin page.
 *
 * Plan §9 Phase 2: "admin adds a local GGUF via the existing drawer
 * with text_embedding=true → picks it in Memory settings → memory
 * works." We don't actually download a GGUF in CI; this scaffold
 * exercises the admin page surface + capability=text_embedding filter.
 */

test.describe('Memory — admin embedding picker', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('admin page exposes capability-filtered model dropdown', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `pick_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::admin::read',
        'memory::admin::manage',
        'llm_models::read',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)

    await page.goto(`${baseURL}/settings/admin/memory`)

    // The capability filter API responds 200 (even with empty list).
    const res = await page.request.get(
      `${apiURL}/api/llm-models?capability=text_embedding&page=1&per_page=10`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    expect(res.status()).toBe(200)

    // Page renders an "embedding model" combobox.
    await expect(page.getByText(/Embedding model/)).toBeVisible()
  })
})
