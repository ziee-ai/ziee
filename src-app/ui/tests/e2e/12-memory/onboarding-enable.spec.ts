import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding Memory step: ENABLE path.
 *
 * Plan §9 Phase 1: "admin walks through onboarding, picks Enable +
 * Download nomic-embed-text from HuggingFace + selects it; assert
 * settings update and the Memories page is reachable."
 *
 * Real-LLM/network gated (requires HuggingFace download). The
 * scaffold below assumes the test fixture has a pre-seeded
 * embedding-capable model so we don't have to run the actual
 * HF download in CI.
 */

const HAS_FIXTURE = Boolean(process.env.MEMORY_E2E_FIXTURE)

test.describe('Memory — onboarding enable', () => {
  test.skip(!HAS_FIXTURE, 'requires MEMORY_E2E_FIXTURE — pre-seeded embedding model')

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('admin enables memory + picks model', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `enable_${Date.now().toString(36)}`
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
        'memory::admin::read',
        'memory::admin::manage',
      ],
    )

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    await page.getByRole('button', { name: /Next/ }).click() // Welcome
    await page.getByRole('button', { name: /Next/ }).click() // API Keys
    await page.getByRole('button', { name: /Next/ }).click() // MCP

    // Memory step: flip switch + pick model.
    await expect(page.getByRole('heading', { name: /Persistent Memory/ })).toBeVisible()
    await page.getByRole('switch').click()
    // Select first option in the model dropdown.
    await page.getByRole('combobox').first().click()
    await page.getByRole('option').first().click()

    await page.getByRole('button', { name: /Next/ }).click()
    await page.getByRole('button', { name: /Finish|Done/ }).click()

    // Verify settings now enabled.
    const adminRes = await page.request.get(`${apiURL}/api/admin/memory-settings`)
    const settings = await adminRes.json()
    expect(settings.enabled).toBe(true)
    expect(settings.embedding_model_id).not.toBeNull()
  })
})
