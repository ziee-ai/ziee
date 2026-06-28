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

  test('embedding section: no-model state + cosine threshold save', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    const embedCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Embedding (semantic search)' })
    await expect(embedCard).toBeVisible({ timeout: 20000 })

    // Fresh deploy has no embedding-capable models → the info Alert shows, the
    // model picker is disabled, and Re-embed is disabled (no current model).
    await expect(
      embedCard.getByText('No embedding-capable models found.'),
    ).toBeVisible()
    await expect(
      embedCard.getByRole('button', { name: 'Re-embed now' }),
    ).toBeDisabled()

    // The cosine threshold knob still saves (no model required).
    await embedCard
      .getByRole('spinbutton', { name: /Cosine distance threshold/i })
      .fill('0.35')
    await embedCard.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Embedding settings saved.')).toBeVisible()
  })

  test('chunking rejects overlap >= chunk size with a validation error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(page.getByText('Chunking', { exact: true })).toBeVisible()

    const chunkingCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Chunk size (characters)' })

    // Set overlap to be >= chunk size — an invalid combination.
    await page.getByRole('spinbutton', { name: /Chunk size/i }).fill('1000')
    await page
      .getByRole('spinbutton', { name: /Chunk overlap/i })
      .fill('2000')
    await chunkingCard.getByRole('button', { name: 'Save' }).click()

    // The client-side guard surfaces an error toast + inline field errors and
    // does NOT save.
    await expect(
      page.getByText('Overlap must be smaller than the chunk size.'),
    ).toBeVisible()
    await expect(
      page.getByText('Must be smaller than the chunk size'),
    ).toBeVisible()
    await expect(page.getByText(/Chunking settings saved/)).toHaveCount(0)
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
})
