/**
 * Hub model download progress — full-width progress bar on the card.
 *
 * When `Hub.createModelFromHub` returns a `DownloadInstance` that
 * already carries `progress_data` (in production the SSE stream
 * pushes updates every second; we shortcut to the initial value),
 * the hub card grows a full-width `<Progress>` bar at the bottom
 * with the computed percent + inline speed/ETA. The top tag flips
 * to a minimal "Downloading" pill.
 *
 * Strategy:
 *   - Seed a local provider so the download flow doesn't bail on
 *     "no local provider found"
 *   - Configure the HF seed repo with a dummy api_key so the pre-
 *     download probe gates pass (the connection-health work just
 *     shipped runs probe + auth gates before the POST fires)
 *   - Mock the test-by-id endpoint to PASS so the probe doesn't
 *     intercept the click
 *   - Mock the download POST to return an instance with
 *     `progress_data: { current: 50, total: 100, ... }`
 *   - Assert the bar is visible at 50% with the speed/ETA info
 */

import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'

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
    throw new Error(`seed provider failed: ${res.status} ${await res.text()}`)
  }
}

async function configureHfWithDummyKey(
  apiURL: string,
  token: string,
): Promise<void> {
  const list = await fetch(`${apiURL}/api/llm-repositories`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())
  const hf = (list.repositories as any[]).find(
    r => r.name === 'Hugging Face Hub',
  )
  if (!hf) throw new Error('HF Hub not seeded')
  const resp = await fetch(`${apiURL}/api/llm-repositories/${hf.id}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      auth_config: { api_key: 'hf-dummy-e2e' },
    }),
  })
  if (!resp.ok) {
    throw new Error(`configure HF failed: ${resp.status}`)
  }
}

/** Mock the by-id probe to PASS so the pre-download gates clear. */
async function mockRepoProbePass(page: Page): Promise<void> {
  await page.route(
    /\/api\/llm-repositories\/[^/]+\/test$/,
    async (route, request) => {
      if (request.method() !== 'POST') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ success: true, message: 'ok' }),
      })
    },
  )
}

/** Mock the download-start POST to return an instance with mid-progress. */
async function mockDownloadStartWithProgress(
  page: Page,
  pathByHubId: Map<string, string>,
): Promise<void> {
  // Once a download starts, the store reloads GET /api/llm-models/downloads
  // (via setupDownloadTracking) and would overwrite the externally-added
  // mocked row with the real (empty) list. Track the started download and
  // serve it from the list endpoint too so it survives the reload.
  let activeDownload: Record<string, unknown> | null = null
  await page.route(/\/api\/llm-models\/downloads(\?|$)/, async route => {
    if (!activeDownload) return route.fallback()
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        downloads: [activeDownload],
        total: 1,
        page: 1,
        per_page: 100,
      }),
    })
  })

  await page.route(/\/api\/hub\/models\/download$/, async (route, request) => {
    if (request.method() !== 'POST') return route.fallback()
    // Resolve the requested model's repository_path so the download links
    // to its card (the card matches by `request_data.repository_path ===
    // model.repository_path`). The POST body carries hub_id, not the path.
    const hubId = request.postDataJSON()?.hub_id as string | undefined
    const reqPath = (hubId && pathByHubId.get(hubId)) || 'mock/repo'
    const now = new Date().toISOString()
    activeDownload = {
      id: '00000000-0000-0000-0000-0000000000d1',
      provider_id: '00000000-0000-0000-0000-0000000000a1',
      repository_id: '00000000-0000-0000-0000-0000000000b1',
      status: 'downloading',
      // Realistic progress_data so the hub card renders the bar at 50%
      // immediately (no need to wait for the SSE stream).
      progress_data: {
        current: 524288000, // 500 MB
        total: 1048576000, // 1 GB
        speed_bps: 5242880, // 5 MB/s
        eta_seconds: 100, // 1m 40s
        phase: null,
        message: null,
      },
      request_data: { repository_path: reqPath },
      error_message: null,
      model_id: null,
      completed_at: null,
      created_at: now,
      started_at: now,
      updated_at: now,
    }
    await route.fulfill({
      status: 201,
      contentType: 'application/json',
      body: JSON.stringify({
        download: activeDownload,
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
}

test('hub card renders a full-width Progress bar at the bottom with percent + speed + ETA', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const token = await getAdminToken(baseURL)
  await seedLocalProvider(baseURL, token, 'E2E Local')
  await configureHfWithDummyKey(baseURL, token)

  await mockRepoProbePass(page)
  // The download POST carries hub_id; the card matches downloads by
  // repository_path, so give the mock a hub_id → repository_path map.
  const hubModels = await fetch(`${baseURL}/api/hub/models?lang=en`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())
  const pathByHubId = new Map<string, string>(
    (hubModels as Array<{ id: string; repository_path: string }>).map(m => [
      m.id,
      m.repository_path,
    ]),
  )
  await mockDownloadStartWithProgress(page, pathByHubId)

  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  const firstCard = (await getModelCards(page)).first()
  const modelName = (await firstCard.getAttribute('data-testid'))!.replace(
    'hub-model-card-',
    '',
  )
  await firstCard.getByTestId(`hub-model-download-btn-${modelName}`).click()

  // Handle the optional Select Quantization dialog.
  const quantModal = page.getByTestId('hub-model-download-quant-dialog')
  if (await quantModal.isVisible({ timeout: 1500 }).catch(() => false)) {
    await page.getByTestId('hub-model-download-quant-dialog-ok-btn').click()
  }

  // The "Download started" toast confirms the POST landed and the
  // store added the row with the mocked progress_data.
  await expect(
    page
      .locator('[data-sonner-toast][data-type="success"]')
      .filter({ hasText: /Download started/i }),
  ).toBeVisible({ timeout: 10_000 })

  // Top status tag flips to "Downloading" (no percent — that lives
  // on the bar).
  const statusTag = firstCard.getByTestId(`hub-model-status-tag-${modelName}`)
  await expect(statusTag).toBeVisible({ timeout: 5_000 })
  await expect(statusTag).toContainText('Downloading')

  // The full-width Progress at the bottom of the card body shows 50%
  // and the inline info contains a speed unit.
  const progress = firstCard.getByTestId(`hub-model-progress-${modelName}`)
  await expect(progress).toBeVisible({ timeout: 5_000 })
  await expect(progress).toContainText('50%')
  await expect(progress).toContainText(/MB\/s/)
})
