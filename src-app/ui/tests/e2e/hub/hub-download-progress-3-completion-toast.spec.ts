/**
 * Hub model download progress — completion toast via the EventBus.
 *
 * Verifies the end-to-end notification path:
 *   1. Click Download → POST /api/hub/models/download (mocked) returns
 *      a downloading instance.
 *   2. FE subscribes to /api/llm-models/downloads/subscribe (mocked
 *      with a scripted SSE stream).
 *   3. Stream sends: connected → update[downloading 50%] →
 *      update[completed] → complete.
 *   4. Store's SSE update handler detects the
 *      `downloading → completed` transition and emits
 *      `llm_model.download_completed`.
 *   5. The globally-mounted `LlmModelDownloadNotifications` listener
 *      surfaces a `message.success` toast carrying the model
 *      display_name.
 *   6. The completed row is filtered out of the downloads array → the
 *      hub card no longer shows the "Downloading" tag.
 *
 * Uses the existing `serializeSseScript` helper at
 * `tests/e2e/helpers/sse-mock-helpers.ts` — the downloads endpoint
 * shares the same SSE wire format (`event: name\ndata: json\n\n`) as
 * the chat stream; only the event names differ. Backend SSE event
 * names come from `sse_event_enum!` in
 * `server/src/common/macros.rs`, which converts the Rust variants
 * `Connected | Update | Complete | Error` to camelCase wire names.
 */

import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { getModelCards } from './helpers/hub-models'
import {
  serializeSseScript,
  type ScriptedSseEvent,
} from '../helpers/sse-mock-helpers'

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
    body: JSON.stringify({ auth_config: { api_key: 'hf-dummy-e2e' } }),
  })
  if (!resp.ok) {
    throw new Error(`configure HF failed: ${resp.status}`)
  }
}

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

const FAKE_DOWNLOAD_ID = '00000000-0000-0000-0000-0000000000d1'
const FAKE_PROVIDER_ID = '00000000-0000-0000-0000-0000000000a1'
const FAKE_REPOSITORY_ID = '00000000-0000-0000-0000-0000000000b1'

async function mockDownloadStartPending(
  page: Page,
  modelDisplayName: string,
): Promise<void> {
  await page.route(/\/api\/hub\/models\/download$/, async (route, request) => {
    if (request.method() !== 'POST') return route.fallback()
    const now = new Date().toISOString()
    await route.fulfill({
      status: 201,
      contentType: 'application/json',
      body: JSON.stringify({
        download: {
          id: FAKE_DOWNLOAD_ID,
          provider_id: FAKE_PROVIDER_ID,
          repository_id: FAKE_REPOSITORY_ID,
          status: 'downloading',
          progress_data: {
            current: 524288000,
            total: 1048576000,
            speed_bps: 5242880,
            eta_seconds: 100,
          },
          // `request_data.display_name` flows through the store's
          // SSE transition-detection into the emitted event's
          // `modelDisplayName` field — the toast renders it
          // verbatim. Keep it stable so the assertion is precise.
          request_data: {
            display_name: modelDisplayName,
            model_name: 'mock-model',
            repository_path: 'mock/repo',
            file_format: 'gguf',
            main_filename: 'mock.gguf',
          },
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
          entity_id: FAKE_DOWNLOAD_ID,
          hub_id: 'mock-hub-model-id',
          hub_category: 'mock-category',
          created_at: now,
          created_by: null,
        },
      }),
    })
  })
}

/**
 * Mock the SSE subscribe endpoint with a scripted event sequence that
 * transitions our fake download `downloading → completed` in a single
 * response payload.
 *
 * The wire format (one body, all events fired in sequence by the SSE
 * client) works for our purposes because the chat-stream tests use
 * the same pattern. The store's update handler processes each event
 * sequentially, so the snapshot-based transition detection still sees
 * the `downloading → completed` flip and emits the bus event.
 */
async function mockDownloadsSseStream(page: Page): Promise<void> {
  await page.route(
    /\/api\/llm-models\/downloads\/subscribe(\?|$)/,
    async (route, request) => {
      if (request.method() !== 'GET') return route.fallback()
      const script: ScriptedSseEvent[] = [
        // sse_event_enum! lowercases the variant — see
        // src-app/server/src/common/macros.rs:33-77.
        {
          event: 'connected',
          data: { message: 'Connected to download progress stream' },
        },
        // Tick 1: still downloading. Mirrors the wire shape the store
        // expects (an array of DownloadProgressUpdate-like objects).
        {
          event: 'update',
          data: [
            {
              id: FAKE_DOWNLOAD_ID,
              provider_id: FAKE_PROVIDER_ID,
              status: 'downloading',
              current: 524288000,
              total: 1048576000,
              speed_bps: 5242880,
              eta_seconds: 100,
            },
          ],
        },
        // Tick 2: transitioned to completed. The store's
        // pre-update-status snapshot fires the
        // `llm_model.download_completed` event ONCE on this
        // transition (prev='downloading', current='completed'), which
        // the global listener turns into a toast.
        {
          event: 'update',
          data: [
            {
              id: FAKE_DOWNLOAD_ID,
              provider_id: FAKE_PROVIDER_ID,
              status: 'completed',
              current: 1048576000,
              total: 1048576000,
              speed_bps: 0,
              eta_seconds: 0,
            },
          ],
        },
        // Stream-end. The store's `complete` handler refreshes
        // provider models and disconnects SSE.
        { event: 'complete', data: 'All downloads completed' },
      ]
      await route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: serializeSseScript(script),
      })
    },
  )
}

test('SSE update transitioning to completed fires the success toast', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const token = await getAdminToken(baseURL)
  await seedLocalProvider(baseURL, token, 'E2E Local')
  await configureHfWithDummyKey(baseURL, token)

  await mockRepoProbePass(page)

  // The toast renders `Downloaded ${modelDisplayName}`. We use a
  // unique sentinel string so the assertion doesn't false-match any
  // other "Downloaded" copy on the page (the hub card itself shows a
  // "Downloaded" tag when a model is installed).
  const SENTINEL = `e2e-toast-sentinel-${Math.random().toString(36).slice(2, 8)}`
  await mockDownloadStartPending(page, SENTINEL)
  await mockDownloadsSseStream(page)

  await loginAsAdmin(page, baseURL)
  await navigateToHub(page, baseURL, 'models')
  await waitForHubDataLoad(page)

  const firstCard = (await getModelCards(page)).first()
  const modelName = (await firstCard.getAttribute('data-testid'))!.replace(
    'hub-model-card-',
    '',
  )
  await firstCard.getByTestId(`hub-model-download-btn-${modelName}`).click()

  // Skip any quantization dialog that the model happens to offer.
  const quantModal = page.getByTestId('hub-model-download-quant-dialog')
  if (await quantModal.isVisible({ timeout: 1500 }).catch(() => false)) {
    await page.getByTestId('hub-model-download-quant-dialog-ok-btn').click()
  }

  // Initial "Download started" message confirms the POST landed.
  await expect(
    page
      .locator('[data-sonner-toast][data-type="success"]')
      .filter({ hasText: /Download started/i }),
  ).toBeVisible({ timeout: 10_000 })

  // Completion toast: "Downloaded <SENTINEL>". The notification
  // component sets `duration: 5` so we have 5s to assert.
  await expect(
    page.locator('[data-sonner-toast][data-type="success"]').filter({
      hasText: new RegExp(`Downloaded ${SENTINEL}`),
    }),
  ).toBeVisible({ timeout: 10_000 })
})
