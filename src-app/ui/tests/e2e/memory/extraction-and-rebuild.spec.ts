import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
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
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-4o-mini',
      'GPT-4o Mini',
      'openai',
    )

    await page.goto(`${baseURL}/settings/memory-admin`)
    const extractionCard = byTestId(page, 'memory-extraction-card')
    await expect(extractionCard).toBeVisible({ timeout: 20000 })

    await byTestId(extractionCard, 'memory-extraction-model-combobox').click()
    await byTestId(page, `memory-extraction-model-combobox-opt-${modelId}`).click()

    const saveResp = page.waitForResponse(
      (r) =>
        /\/api\/memory\/admin-settings$/.test(r.url()) &&
        r.request().method() === 'PUT' &&
        r.ok(),
    )
    await byTestId(extractionCard, 'memory-extraction-save-btn').click()
    await saveResp
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
    await expect(byTestId(page, 'memory-rebuild-fts-card')).toBeVisible({
      timeout: 20000,
    })

    // It keeps polling the status endpoint (2s interval) — at least one
    // additional poll lands after the initial load.
    const first = ftsStatusCalls
    await expect
      .poll(() => ftsStatusCalls, { timeout: 15000 })
      .toBeGreaterThan(first)
  })
})
