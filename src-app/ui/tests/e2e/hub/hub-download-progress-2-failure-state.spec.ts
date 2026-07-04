/**
 * Hub model download progress — failure state on the card.
 *
 * When a download has `status: 'failed'`, the store keeps the row
 * around (unlike completed/cancelled which it filters out), so the
 * hub card can render a failure state. This spec verifies the card's
 * three failure affordances:
 *   - Top tag flips to red "Download Failed"
 *   - Progress bar wears `status="exception"` (red, no animation)
 *   - Retry button visible under the bar
 *
 * We inject the failed DownloadInstance directly into the store via
 * `page.evaluate` instead of trying to mock an SSE-driven failure —
 * mocking long-lived SSE streams in Playwright is fragile, and the
 * store action is the same path the SSE handler uses to populate the
 * array. The UI-side rendering is what this spec actually exercises.
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'

test('hub card surfaces failure state with red tag, exception bar, and Retry button', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  // Read the first RENDERED card's model id so we can construct a
  // synthetic DownloadInstance whose `request_data.repository_path`
  // matches the model's `repository_path`. The hub card's filter
  // joins on `repository_path`, so the failed row only attaches when
  // they match. Note: the store's models[0] is NOT necessarily the
  // first rendered card (e.g. a 70B entry with no downloadable variant
  // is in the catalog but not rendered), so resolve the path from THIS
  // card's id via the hub API rather than guessing the store index.
  const firstCard = (await getModelCards(page)).first()
  await firstCard.waitFor({ state: 'visible', timeout: 10_000 })

  const cardModelId =
    (await firstCard.getAttribute('data-testid'))?.replace(
      'hub-model-card-',
      '',
    ) ?? ''
  expect(cardModelId).not.toBe('')

  // v2 Phase 7: models are keyed by reverse-DNS `name` (matching the
  // card testid) and the download `repository_path` comes off a
  // source's `identifier` (there is no model-wide `repository_path`).
  const token = await getAdminToken(baseURL)
  const hubModels = (await fetch(`${baseURL}/api/hub/models?lang=en`, {
    headers: { Authorization: `Bearer ${token}` },
  }).then(r => r.json())) as Array<{
    name: string
    sources?: Array<{ identifier: string }>
  }>
  const repoPath =
    hubModels.find(m => m.name === cardModelId)?.sources?.[0]?.identifier ?? ''
  expect(repoPath).not.toBe('')

  // Inject a synthetic FAILED download whose `request_data.repository_path`
  // matches the first card's model by serving it from the downloads list
  // endpoint. The store's `__init__` loads `GET /api/llm-models/downloads`
  // on mount and KEEPS rows with status pending/downloading/failed
  // (LlmModelDownload.store.ts:70), so a mocked `failed` row populates the
  // store through the real data path. (The store is not exposed on
  // `window`, so a direct setState injection is a no-op.) Register the route
  // then reload so the re-mounted store re-fetches through the mock.
  const now = new Date().toISOString()
  const failedDownload = {
    id: '00000000-0000-0000-0000-00000000fa11',
    provider_id: '00000000-0000-0000-0000-0000000000a1',
    repository_id: '00000000-0000-0000-0000-0000000000b1',
    status: 'failed',
    progress_data: {
      current: 256000000,
      total: 1048576000,
      speed_bps: 0,
      eta_seconds: 0,
      phase: null,
      message: null,
    },
    request_data: { repository_path: repoPath, model_name: 'mock-model' },
    error_message: 'HTTP request failed with status: 503',
    model_id: null,
    completed_at: now,
    created_at: now,
    started_at: now,
    updated_at: now,
  }
  await page.route(/\/api\/llm-models\/downloads(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        downloads: [failedDownload],
        total: 1,
        page: 1,
        per_page: 100,
      }),
    })
  })

  await page.reload()
  await waitForHubDataLoad(page)

  // The card's filter picks up the failed row + renders failure state:
  // the top status tag flips to the error "Download Failed" tag.
  const statusTag = firstCard.getByTestId(
    `hub-model-status-tag-${cardModelId}`,
  )
  await expect(statusTag).toBeVisible({ timeout: 5_000 })
  await expect(statusTag).toContainText('Download Failed')

  // The dedicated failed-progress bar (tone="error") renders only in the
  // failure state — its presence is the exception affordance.
  const progress = firstCard.getByTestId(
    `hub-model-failed-progress-${cardModelId}`,
  )
  await expect(progress).toBeVisible({ timeout: 5_000 })
  // Inline reason is clipped to ~50 chars; full reason is in the tooltip
  // (we just check the visible truncation contains the error word).
  await expect(progress).toContainText(/failed|503/i)

  // Retry button appears under the bar.
  await expect(
    firstCard.getByTestId(`hub-model-retry-btn-${cardModelId}`),
  ).toBeVisible({ timeout: 5_000 })

  // Primary Download button is hidden — the user only sees Retry as the
  // forward action when a download has failed.
  await expect(
    firstCard.getByTestId(`hub-model-download-btn-${cardModelId}`),
  ).toHaveCount(0)
})
