import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'

/**
 * E2E for the hub model → local-provider download flow added in the hub
 * cherry-pick. `ModelHubCard.handleDownload`:
 *   - 0 local providers → antd `message.error("No local provider found…")`
 *   - exactly 1        → auto-picks it, starts the download
 *   - >1               → opens a "Select Local Provider" modal first
 *
 * Driver: seed real enabled local providers via the API to control the count,
 * and mock ONLY the download-start POST (/api/hub/models/download) so no
 * GB-scale model download runs. The local-providers list itself is served by
 * the real backend.
 */

async function seedLocalProvider(
  apiURL: string,
  token: string,
  name: string,
): Promise<void> {
  const res = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ name, provider_type: 'local', enabled: true }),
  })
  if (!res.ok) {
    throw new Error(
      `seedLocalProvider(${name}) failed: ${res.status} ${await res.text()}`,
    )
  }
}

/** Configure the seeded "Hugging Face Hub" repository with a dummy
 *  API key so `useHubModelDownloadGate`'s `auth_required &&
 *  !source_auth_configured` branch doesn't fire. The bundled hub seed
 *  models all live on HF and the gate's "Authentication Required"
 *  modal opens BEFORE the download POST. The actual download POST is
 *  mocked by `mockDownloadStart` so the dummy key is never used. */
async function configureHfAuth(apiURL: string, token: string): Promise<void> {
  const listRes = await fetch(`${apiURL}/api/llm-repositories`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!listRes.ok) {
    throw new Error(`list llm-repositories failed: ${listRes.status}`)
  }
  const list = await listRes.json()
  const repos: Array<{ id: string; url: string }> =
    list.repositories ?? list
  const hf = repos.find(r => r.url === 'https://huggingface.co')
  if (!hf) throw new Error('Hugging Face Hub repo not found in seeded data')

  const putRes = await fetch(`${apiURL}/api/llm-repositories/${hf.id}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      auth_config: { api_key: 'test-key-for-gate-only' },
    }),
  })
  if (!putRes.ok) {
    throw new Error(`configure HF auth failed: ${putRes.status} ${await putRes.text()}`)
  }
}

/** Mock the LLM-repository probe endpoint so the download gate's
 *  Gate 2 ("connection probe") returns success without a real network
 *  hit. Pairs with `configureHfAuth` — the auth key satisfies the
 *  "auth configured" check, this satisfies the "repo reachable" check. */
async function mockRepoConnectionProbe(page: Page): Promise<void> {
  await page.route(/\/api\/llm-repositories\/[^/]+\/test$/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ success: true, message: 'mocked OK' }),
    })
  })
}

/** Mock the download-start endpoint. Returns a getter for the hit count. */
async function mockDownloadStart(page: Page): Promise<() => number> {
  let hits = 0
  await page.route(/\/api\/hub\/models\/download$/, async (route, request) => {
    if (request.method() !== 'POST') return route.fallback()
    hits++
    const now = new Date().toISOString()
    await route.fulfill({
      status: 201,
      contentType: 'application/json',
      // Shape must match backend `ModelFromHubResponse`: `download` is
      // a full `DownloadInstance` (with optional fields) and
      // `hub_tracking` is a full `HubEntity` record (NOT an empty
      // object — every UI read of the fields would crash).
      body: JSON.stringify({
        download: {
          id: '00000000-0000-0000-0000-0000000000d1',
          provider_id: '00000000-0000-0000-0000-0000000000a1',
          repository_id: '00000000-0000-0000-0000-0000000000b1',
          status: 'pending',
          request_data: { repository_path: 'mock/repo' },
          progress_data: null,
          error_message: null,
          model_id: null,
          completed_at: null,
          created_at: now,
          started_at: now,
          updated_at: now,
        },
        hub_tracking: {
          id: '00000000-0000-0000-0000-0000000000e1',
          entity_type: 'llm_model',
          entity_id: '00000000-0000-0000-0000-0000000000d1',
          hub_id: 'mock-hub-model-id',
          hub_category: 'mock-category',
          created_at: now,
          created_by: null,
        },
      }),
    })
  })
  return () => hits
}

/** Click the first model card's Download button, then clear any quantization
 *  modal that appears (some seeded models offer multiple quantizations). */
async function clickDownloadOnFirstCard(page: Page) {
  const firstCard = (await getModelCards(page)).first()
  await expect(firstCard).toBeVisible()
  const name =
    (await firstCard.getAttribute('data-testid'))?.replace(
      'hub-model-card-',
      '',
    ) || ''
  await firstCard.getByTestId(`hub-model-download-btn-${name}`).click()

  // A "Select Quantization" dialog appears only for models with >1 option.
  const quantDialog = page.getByTestId('hub-model-download-quant-dialog')
  if (await quantDialog.isVisible({ timeout: 1500 }).catch(() => false)) {
    await page.getByTestId('hub-model-download-quant-dialog-ok-btn').click()
  }
}

test.describe('Hub Model Download — local providers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    // Bundled hub seed models all live on HF. The download gate's
    // "Authentication Required" modal would otherwise fire before
    // the mocked download POST has a chance to register a hit.
    const token = await getAdminToken(testInfra.apiURL)
    await configureHfAuth(testInfra.apiURL, token)
  })

  test('no local provider → shows error, no download starts', async ({ page, testInfra }) => {
    await mockRepoConnectionProbe(page)
    const downloadHits = await mockDownloadStart(page)

    // Seed nothing: a fresh DB has no *enabled* local provider (the built-in
    // 'Local' is disabled), so the list endpoint returns empty.
    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    await expect(
      page
        .locator('[data-sonner-toast][data-type="error"]')
        .filter({ hasText: /No local provider found/i }),
    ).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(0)
  })

  test('single local provider → download starts', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local Solo')
    await mockRepoConnectionProbe(page)
    const downloadHits = await mockDownloadStart(page)

    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    // Exactly one provider → no provider-select modal; download starts directly.
    await expect(
      page
        .locator('[data-sonner-toast][data-type="success"]')
        .filter({ hasText: /Download started/i }),
    ).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(1)
  })

  test('multiple local providers → select modal then download', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local One')
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local Two')
    await mockRepoConnectionProbe(page)
    const downloadHits = await mockDownloadStart(page)

    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    // >1 providers → a "Select Local Provider" dialog must appear first.
    const providerDialog = page.getByTestId('hub-model-download-provider-dialog')
    await expect(providerDialog).toBeVisible({ timeout: 5000 })
    await page.getByTestId('hub-model-download-provider-dialog-ok-btn').click()

    await expect(
      page
        .locator('[data-sonner-toast][data-type="success"]')
        .filter({ hasText: /Download started/i }),
    ).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(1)
  })
})
