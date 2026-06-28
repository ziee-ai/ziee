import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — Document RAG (file_rag) admin settings surface.
 *
 * Embedder-free: exercises the admin page chrome (all section cards render,
 * the capability-filtered embedding picker is present, the sidebar entry
 * exists, and a tuning save round-trips). The grounded-answer flow needs a
 * real embedder and lives in the Tier-3 backend tests.
 */

test.describe('Document RAG — admin settings surface', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('admin page renders all section cards + embedding picker', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `frag_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'file_rag::admin::read',
        'file_rag::admin::manage',
        'llm_models::read',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)

    await page.goto(`${baseURL}/settings/file-rag-admin`)

    // Default is ON, so the settings load and the section cards render. Match
    // the card *titles* exactly — the page subtitle + section notes also
    // mention "chunking"/"full-text" etc., so a loose substring match is
    // ambiguous (strict-mode violation).
    await expect(page.getByText('Document search', { exact: true })).toBeVisible()
    await expect(
      page.getByText('Embedding (semantic search)', { exact: true }),
    ).toBeVisible()
    await expect(page.getByText('Chunking', { exact: true })).toBeVisible()
    await expect(page.getByText('Full-text search', { exact: true })).toBeVisible()
    await expect(page.getByText('Maintenance', { exact: true })).toBeVisible()

    // The capability-filtered embedding model endpoint is reachable.
    const res = await page.request.get(
      `${apiURL}/api/llm-models?capability=text_embedding&page=1&per_page=10`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    expect(res.status()).toBe(200)

    // Embedding-model picker is present.
    await expect(page.getByText(/Embedding model/)).toBeVisible()
  })

  test('chunking settings save round-trips', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `fragsave_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'file_rag::admin::read',
        'file_rag::admin::manage',
        'llm_models::read',
      ],
    )
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(page.getByText('Chunking', { exact: true })).toBeVisible()

    // Change "Chunk size" and save the chunking card; expect a success toast.
    const chunkInput = page.getByRole('spinbutton', { name: /Chunk size/i })
    await chunkInput.fill('1500')
    // The chunking card's own Save button (scope the click to that card).
    const chunkingCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Chunk size (characters)' })
    await chunkingCard.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText(/Chunking settings saved/)).toBeVisible()
  })

  async function ragAdmin(page: import('@playwright/test').Page, baseURL: string, apiURL: string) {
    const adminToken = await getAdminToken(apiURL)
    const username = `frag2_${Date.now().toString(36)}`
    await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
      'profile::read',
      'profile::edit',
      'file_rag::admin::read',
      'file_rag::admin::manage',
      'llm_models::read',
    ])
    await login(page, baseURL, username, 'password123')
    await page.goto(`${baseURL}/settings/file-rag-admin`)
  }

  // EnableSection (Document search master toggle + default top-K) — zero E2E.
  test('EnableSection toggle + default top-K save round-trips', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await ragAdmin(page, baseURL, apiURL)

    const card = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Document search"))',
    )
    await expect(card).toBeVisible({ timeout: 30000 })

    // Flip the deployment-wide enable Switch + change default top-K, then save.
    const enable = card.getByRole('switch', {
      name: 'Enable Document RAG deployment-wide',
    })
    const before = await enable.getAttribute('aria-checked')
    await enable.click()
    const topK = card.getByLabel('Default top-K')
    await topK.click()
    await topK.press('ControlOrMeta+a')
    await topK.fill('7')
    await card.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByText('Document search settings saved.')).toBeVisible({
      timeout: 10000,
    })

    // Reload → both the toggle and top-K persisted.
    await page.goto(`${baseURL}/settings/file-rag-admin`)
    const card2 = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Document search"))',
    )
    await expect(
      card2.getByRole('switch', { name: 'Enable Document RAG deployment-wide' }),
    ).toHaveAttribute('aria-checked', before === 'true' ? 'false' : 'true', {
      timeout: 30000,
    })
    await expect(card2.getByLabel('Default top-K')).toHaveValue('7')
  })

  // Broaden beyond the chunking card — the Maintenance card's backfill action.
  test('Maintenance "Run backfill" dispatches the backfill job', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await ragAdmin(page, baseURL, apiURL)

    const card = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Maintenance"))',
    )
    await expect(card).toBeVisible({ timeout: 30000 })
    const backfill = card.getByTestId('backfill-button')
    await expect(backfill).toBeEnabled()
    await backfill.click()
    await expect(
      page.getByText('Backfill dispatched in the background.'),
    ).toBeVisible({ timeout: 10000 })
  })
})
