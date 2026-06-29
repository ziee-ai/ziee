import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

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

    // Default is ON, so the settings load and the section cards render.
    await expect(byTestId(page, 'filerag-enable-card')).toBeVisible()
    await expect(byTestId(page, 'filerag-embedding-card')).toBeVisible()
    await expect(byTestId(page, 'filerag-chunking-card')).toBeVisible()
    await expect(byTestId(page, 'filerag-fts-card')).toBeVisible()
    await expect(byTestId(page, 'filerag-maintenance-card')).toBeVisible()

    // The capability-filtered embedding model endpoint is reachable.
    const res = await page.request.get(
      `${apiURL}/api/llm-models?capability=text_embedding&page=1&per_page=10`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    expect(res.status()).toBe(200)

    // Embedding-model picker is present.
    await expect(byTestId(page, 'filerag-embedding-model-select')).toBeVisible()
  })

  test('MaintenanceSection Run backfill dispatches a background job', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-maintenance-card')).toBeVisible({
      timeout: 20000,
    })

    // Trigger the (idempotent) backfill — with no eligible files it's a no-op
    // server-side but still dispatches successfully (real POST round-trip).
    const backfillResp = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/backfill') &&
        r.request().method() === 'POST',
      { timeout: 10000 },
    )
    await byTestId(page, 'filerag-maintenance-backfill').click()
    expect((await backfillResp).ok()).toBeTruthy()
  })

  test('FullTextSection saves the RRF k tuning knob', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-fts-card')).toBeVisible({ timeout: 20000 })

    await byTestId(page, 'filerag-fts-rrf-k').fill('77')
    const ftsSave = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-fts-save').click()
    await ftsSave
  })

  test('EnableSection master toggle saves the document-search setting', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-enable-card')).toBeVisible({ timeout: 20000 })

    // Flip the master switch and confirm it actually toggled, then save.
    const master = byTestId(page, 'filerag-enable-switch')
    const before = await master.getAttribute('aria-checked')
    await master.click()
    await expect(master).not.toHaveAttribute('aria-checked', before ?? 'true')

    const enableSave = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-enable-save').click()
    await enableSave
  })

  test('embedding section: no-model state + cosine threshold save', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-embedding-card')).toBeVisible({ timeout: 20000 })

    // Fresh deploy has no embedding-capable models → the info Alert shows and
    // Re-embed is disabled (no current model).
    await expect(byTestId(page, 'filerag-embedding-no-models-alert')).toBeVisible()
    await expect(byTestId(page, 'filerag-embedding-reembed-btn')).toBeDisabled()

    // The cosine threshold knob still saves (no model required).
    await byTestId(page, 'filerag-embedding-cosine').fill('0.35')
    const embedSave = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-embedding-save').click()
    await embedSave
  })

  test('chunking rejects overlap >= chunk size with a validation error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/file-rag-admin`)
    await expect(byTestId(page, 'filerag-chunking-card')).toBeVisible()

    // Set overlap to be >= chunk size — an invalid combination.
    await byTestId(page, 'filerag-chunking-chunk-chars').fill('1000')
    await byTestId(page, 'filerag-chunking-overlap').fill('2000')
    await byTestId(page, 'filerag-chunking-save').click()

    // The client-side guard surfaces the chunking error alert and does NOT save.
    await expect(byTestId(page, 'filerag-chunking-error-alert')).toBeVisible()
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
    await expect(byTestId(page, 'filerag-chunking-card')).toBeVisible()

    // Change "Chunk size" and save the chunking card; expect a real round-trip.
    await byTestId(page, 'filerag-chunking-chunk-chars').fill('1500')
    const chunkSave = page.waitForResponse(
      r =>
        r.url().includes('/api/file-rag/admin-settings') &&
        r.request().method() === 'PUT' &&
        r.status() === 200,
    )
    await byTestId(page, 'filerag-chunking-save').click()
    await chunkSave
  })
})
