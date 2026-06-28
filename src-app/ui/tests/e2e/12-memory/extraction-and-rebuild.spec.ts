import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Memory admin — two previously-uncovered section cards on
 * /settings/memory-admin:
 *   - ExtractionSection: pick a default extraction model + Save.
 *   - RebuildStatusSection: self-shows + polls while a rebuild is in flight.
 */
test.describe('Memory admin — extraction + rebuild status', () => {
  test('ExtractionSection saves a chosen default extraction model', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // A chat-capable model must exist for the extraction picker to offer one.
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-4o-mini',
      'GPT-4o Mini',
      'openai',
    )

    await page.goto(`${baseURL}/settings/memory-admin`)
    const extractionCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Default extraction model' })
    await expect(extractionCard).toBeVisible({ timeout: 20000 })

    await extractionCard.locator('.ant-select').click()
    await page.getByRole('option', { name: 'GPT-4o Mini' }).click()
    await extractionCard.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Extraction settings saved.')).toBeVisible()
  })

  test('RebuildStatusSection shows + polls while an FTS rebuild is in flight', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Embedding rebuild: not running. FTS rebuild: in progress → the section
    // un-hides and polls the status endpoint every 2s.
    await page.route(
      /\/api\/memory\/admin-settings\/rebuild-status$/,
      async route =>
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ in_progress: false, pending_count: 0 }),
        }),
    )
    let ftsStatusCalls = 0
    await page.route(
      /\/api\/memory\/admin\/fts\/rebuild\/status$/,
      async route => {
        ftsStatusCalls += 1
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            in_progress: true,
            started_at: '2026-06-01T00:00:00Z',
          }),
        })
      },
    )

    await page.goto(`${baseURL}/settings/memory-admin`)

    // The in-flight FTS rebuild card surfaces.
    await expect(
      page.getByText('Rebuilding full-text search index'),
    ).toBeVisible({ timeout: 20000 })

    // It keeps polling the status endpoint (2s interval) — at least one
    // additional poll lands after the initial load.
    const first = ftsStatusCalls
    await expect
      .poll(() => ftsStatusCalls, { timeout: 15000 })
      .toBeGreaterThan(first)
  })
})
