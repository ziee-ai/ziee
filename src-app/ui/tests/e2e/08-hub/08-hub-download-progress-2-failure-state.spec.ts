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
import { loginAsAdmin } from '../../common/auth-helpers'
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

  // Read the first card's `data-model-id` so we can construct a
  // synthetic DownloadInstance whose `request_data.repository_path`
  // matches the model's `repository_path`. The hub card's filter
  // joins on `repository_path`, so the failed row only attaches when
  // they match.
  const firstCard = (await getModelCards(page)).first()
  await firstCard.waitFor({ state: 'visible', timeout: 10_000 })

  // Pull the model's repository_path out of the rendered model
  // metadata (Stores.HubModels exposes the catalog list to window).
  const repoPath: string = await page.evaluate(() => {
    // Stores is the global proxy shape installed by the core module.
    const w = window as any
    const models = w?.Stores?.HubModels?.__state?.models ?? []
    return models[0]?.repository_path ?? ''
  })
  expect(repoPath).not.toBe('')

  // Inject a synthetic FAILED download whose `request_data.repository_path`
  // matches the first card's model. The store's `__state` accessor bypasses
  // the proxy's render-only guard (per the `feedback_stores_state_in_handlers`
  // codebase convention) so we can mutate from a test driver.
  await page.evaluate(repoPath => {
    const w = window as any
    const store = w?.Stores?.LlmModelDownload?.__state
    if (!store?.downloads) return
    const now = new Date().toISOString()
    const fakeDownload = {
      id: '00000000-0000-0000-0000-00000000fa11',
      provider_id: '00000000-0000-0000-0000-0000000000a1',
      repository_id: '00000000-0000-0000-0000-0000000000b1',
      status: 'failed',
      progress_data: {
        current: 256000000,
        total: 1048576000,
        speed_bps: 0,
        eta_seconds: 0,
      },
      request_data: { repository_path: repoPath, model_name: 'mock-model' },
      error_message: 'HTTP request failed with status: 503',
      model_id: null,
      completed_at: now,
      created_at: now,
      started_at: now,
      updated_at: now,
    }
    // Push directly into the zustand store's underlying state.
    // useLlmModelDownloadStore.setState merges into existing state.
    w.useLlmModelDownloadStore?.setState((s: any) => ({
      downloads: [...s.downloads, fakeDownload],
    }))
  }, repoPath)

  // The card's filter picks up the failed row + renders failure state.
  await expect(
    firstCard.locator('.ant-tag').filter({ hasText: 'Download Failed' }),
  ).toBeVisible({ timeout: 5_000 })

  const progress = firstCard.locator('.ant-progress')
  await expect(progress).toBeVisible({ timeout: 5_000 })
  // antd applies `.ant-progress-status-exception` for `status="exception"`.
  await expect(progress).toHaveClass(/ant-progress-status-exception/)
  // Inline reason is clipped to ~50 chars; full reason is in the tooltip
  // (we just check the visible truncation contains the error word).
  await expect(progress).toContainText(/failed|503/i)

  // Retry button appears under the bar.
  await expect(
    firstCard.getByRole('button', { name: /retry/i }),
  ).toBeVisible({ timeout: 5_000 })

  // Primary Download button is hidden — the user only sees Retry as the
  // forward action when a download has failed.
  await expect(
    firstCard.getByRole('button', { name: /^download$/i }),
  ).toHaveCount(0)
})
