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
      body: JSON.stringify({
        download: {
          id: '00000000-0000-0000-0000-0000000000d1',
          provider_id: '00000000-0000-0000-0000-0000000000a1',
          repository_id: '00000000-0000-0000-0000-0000000000b1',
          status: 'pending',
          request_data: { repository_path: 'mock/repo' },
          created_at: now,
          started_at: now,
          updated_at: now,
        },
        hub_tracking: {},
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
  await firstCard.getByRole('button', { name: /download/i }).click()

  // A "Select Quantization" modal appears only for models with >1 option.
  const quantModal = page.getByRole('dialog').filter({ hasText: 'Select Quantization' })
  if (await quantModal.isVisible({ timeout: 1500 }).catch(() => false)) {
    await quantModal.getByRole('button', { name: 'Continue' }).click()
  }
}

test.describe('Hub Model Download — local providers', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('no local provider → shows error, no download starts', async ({ page, testInfra }) => {
    const downloadHits = await mockDownloadStart(page)

    // Seed nothing: a fresh DB has no *enabled* local provider (the built-in
    // 'Local' is disabled), so the list endpoint returns empty.
    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    await expect(page.getByText(/No local provider found/i)).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(0)
  })

  test('single local provider → download starts', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local Solo')
    const downloadHits = await mockDownloadStart(page)

    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    // Exactly one provider → no provider-select modal; download starts directly.
    await expect(page.getByText(/Download started/i)).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(1)
  })

  test('multiple local providers → select modal then download', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local One')
    await seedLocalProvider(testInfra.apiURL, token, 'E2E Local Two')
    const downloadHits = await mockDownloadStart(page)

    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    await clickDownloadOnFirstCard(page)

    // >1 providers → a "Select Local Provider" modal must appear first.
    const providerModal = page.getByRole('dialog').filter({ hasText: 'Select Local Provider' })
    await expect(providerModal).toBeVisible({ timeout: 5000 })
    await providerModal.getByRole('button', { name: 'Continue' }).click()

    await expect(page.getByText(/Download started/i)).toBeVisible({ timeout: 5000 })
    expect(downloadHits()).toBe(1)
  })
})
