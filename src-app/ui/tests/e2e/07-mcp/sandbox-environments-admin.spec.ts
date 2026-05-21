import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

// ---------------------------------------------------------------------------
// Route mocks. The real prefetch endpoint downloads a GB-scale rootfs from a
// network mirror — non-deterministic + slow — so we mock all three
// /api/code-sandbox/* endpoints with deterministic responses. The SSE wire
// format matches the backend's committed SSEPrefetchEvent union exactly.
// ---------------------------------------------------------------------------

const SSE_HEADERS = { contentType: 'text/event-stream' }

/** environments mock; `cachedFull` flips after a successful fetch. */
async function mockEnvironments(page: Page, getCachedFull: () => boolean) {
  await page.route(/\/api\/code-sandbox\/environments$/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        available: [
          {
            flavor: 'minimal',
            description: 'Shell + python3',
            approximate_size_mb: 57,
            cached: true,
          },
          {
            flavor: 'full',
            description: 'numpy + torch + R + Node',
            approximate_size_mb: 853,
            cached: getCachedFull(),
          },
        ],
      }),
    })
  })
}

/** POST starts a task; GET lists tasks (default: none running). */
async function mockPrefetchListAndStart(
  page: Page,
  opts: { onStart?: () => void; runningTasks?: () => unknown[] } = {},
) {
  await page.route(/\/api\/code-sandbox\/prefetch$/, async route => {
    if (route.request().method() === 'POST') {
      opts.onStart?.()
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          task_id: '00000000-0000-0000-0000-000000000001',
          flavor: 'full',
          expected_size_mb: 853,
          status: 'running',
          events_url: '/api/code-sandbox/prefetch/full/events',
        }),
      })
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ tasks: opts.runningTasks?.() ?? [] }),
    })
  })
}

/** SSE stream body builder. */
function sseBody(events: Array<{ event: string; data: unknown }>): string {
  return (
    events
      .map(e => `event: ${e.event}\ndata: ${JSON.stringify(e.data)}\n\n`)
      .join('')
  )
}

async function mockEventsStream(
  page: Page,
  events: Array<{ event: string; data: unknown }>,
) {
  await page.route(
    /\/api\/code-sandbox\/prefetch\/full\/events$/,
    async route => {
      await route.fulfill({
        status: 200,
        ...SSE_HEADERS,
        body: sseBody(events),
      })
    },
  )
}

const CONNECTED = {
  event: 'connected',
  data: {
    flavor: 'full',
    task_id: '00000000-0000-0000-0000-000000000001',
    status: 'running',
    expected_size_mb: 853,
  },
}
const PROGRESS_DL = {
  event: 'progress',
  data: { phase: 'downloading', message: 'downloading full' },
}
const PROGRESS_INSTALL = {
  event: 'progress',
  data: { phase: 'installing', message: 'installing' },
}
const COMPLETE = {
  event: 'complete',
  data: { bytes_downloaded: 853000000, duration_ms: 1200, cosign_verified: false },
}

async function gotoSandboxEnvironments(page: Page, baseURL: string) {
  // Vite's dev server can return a transient 504 "Outdated Optimize Dep"
  // when it re-optimizes deps mid-navigation (common on cold-cache
  // parallel test startup). The only recovery is a reload, so retry the
  // navigation up to 3 times until the page heading renders.
  const heading = page.getByRole('heading', { name: 'Sandbox Environments' })
  for (let attempt = 1; attempt <= 3; attempt++) {
    await page.goto(`${baseURL}/settings/sandbox-environments`)
    await page.waitForLoadState('networkidle').catch(() => {})
    try {
      await expect(heading).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      // 504 re-optimization in flight — reload and retry.
      await page.waitForTimeout(1000)
    }
  }
}

function fullRow(page: Page) {
  return page.locator('tr[data-flavor="full"]')
}

// ---------------------------------------------------------------------------

test.describe('Sandbox Environments admin settings', () => {
  // Vite's dev server occasionally returns a transient 504
  // "Outdated Optimize Dep" during cold-cache parallel startup (it
  // re-bundles deps mid-navigation, breaking an in-flight chunk
  // load). The recovery is a fresh attempt against a now-warm cache.
  // Mirror the precedent in 09-chat/chat-right-panel.spec.ts.
  test.describe.configure({ retries: 2 })

  test('lists environments with cached status', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockEnvironments(page, () => false)
    await mockPrefetchListAndStart(page)
    await gotoSandboxEnvironments(page, baseURL)

    // minimal is cached → Cached tag, no Fetch button.
    const minimalRow = page.locator('tr[data-flavor="minimal"]')
    await expect(minimalRow.getByText('Cached')).toBeVisible()

    // full is not cached → Not fetched + a Fetch button.
    await expect(fullRow(page).getByText('Not fetched')).toBeVisible()
    await expect(
      fullRow(page).getByRole('button', { name: 'Fetch' }),
    ).toBeVisible()
  })

  test('fetch shows progress bar while running', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockEnvironments(page, () => false)
    await mockPrefetchListAndStart(page)
    // Stalled stream: connected + progress, NO terminal event — the row
    // stays in the running state so the progress bar is reliably visible.
    await mockEventsStream(page, [CONNECTED, PROGRESS_DL])
    await gotoSandboxEnvironments(page, baseURL)

    await fullRow(page).getByRole('button', { name: 'Fetch' }).click()
    await expect(
      fullRow(page).getByTestId('prefetch-progress'),
    ).toBeVisible({ timeout: 10000 })
  })

  test('fetch completes and flips the row to cached', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    let cachedFull = false
    await loginAsAdmin(page, baseURL)
    await mockEnvironments(page, () => cachedFull)
    await mockPrefetchListAndStart(page, { onStart: () => { cachedFull = true } })
    await mockEventsStream(page, [CONNECTED, PROGRESS_DL, PROGRESS_INSTALL, COMPLETE])
    await gotoSandboxEnvironments(page, baseURL)

    await fullRow(page).getByRole('button', { name: 'Fetch' }).click()

    // The `complete` event + the post-complete loadEnvironments reload flip
    // the full row to Cached. Reaching Cached proves the whole SSE pipeline
    // (connect → progress → complete → reload) worked.
    await expect(fullRow(page).getByText('Cached')).toBeVisible({ timeout: 15000 })
    await expect(
      fullRow(page).getByRole('button', { name: 'Fetch' }),
    ).toHaveCount(0)
  })

  test('failed fetch shows error and keeps the Fetch button', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockEnvironments(page, () => false)
    await mockPrefetchListAndStart(page)
    await mockEventsStream(page, [
      CONNECTED,
      { event: 'failed', data: { error: 'network unreachable' } },
    ])
    await gotoSandboxEnvironments(page, baseURL)

    await fullRow(page).getByRole('button', { name: 'Fetch' }).click()
    await expect(fullRow(page).getByText(/Failed: network unreachable/)).toBeVisible({
      timeout: 10000,
    })
    // Retry still possible — button stays.
    await expect(
      fullRow(page).getByRole('button', { name: 'Fetch' }),
    ).toBeVisible()
  })

  test('reload resumes an in-progress fetch (no click needed)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    let cachedFull = false
    await loginAsAdmin(page, baseURL)
    await mockEnvironments(page, () => cachedFull)
    // GET /prefetch reports `full` already running (started elsewhere /
    // before reload). The store's resumeRunningTasks re-subscribes to its
    // SSE on load WITHOUT any click.
    await mockPrefetchListAndStart(page, {
      runningTasks: () => [
        {
          task_id: '00000000-0000-0000-0000-000000000001',
          flavor: 'full',
          status: 'running',
          started_at: new Date().toISOString(),
          last_phase: 'downloading',
        },
      ],
    })
    // The resumed SSE stream then completes.
    await mockEventsStream(page, [CONNECTED, PROGRESS_INSTALL, COMPLETE])
    // flip cached when the (resumed) stream completes → loadEnvironments reload
    await page.route(/\/api\/code-sandbox\/prefetch\/full\/events$/, async route => {
      cachedFull = true
      await route.fulfill({
        status: 200,
        ...SSE_HEADERS,
        body: sseBody([CONNECTED, PROGRESS_INSTALL, COMPLETE]),
      })
    })
    await gotoSandboxEnvironments(page, baseURL)

    // Without clicking Fetch, the full row resumes + reaches Cached.
    await expect(fullRow(page).getByText('Cached')).toBeVisible({ timeout: 15000 })
  })

  test('manage permission gates the Fetch button', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // Create the admin on the fresh test DB (via /setup) so getAdminToken
    // can authenticate; then create a user with ONLY read perm.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `sbx_read_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@example.com`, 'password123', [
      'code_sandbox::environments::read',
    ])
    await login(page, baseURL, uname, 'password123')

    await mockEnvironments(page, () => false)
    await mockPrefetchListAndStart(page)
    await gotoSandboxEnvironments(page, baseURL)

    // Table renders (read allowed) but the Fetch button is disabled.
    await expect(fullRow(page).getByText('Not fetched')).toBeVisible()
    await expect(
      fullRow(page).getByRole('button', { name: 'Fetch' }),
    ).toBeDisabled()
  })

  test('read permission gates the whole page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // Create the admin on the fresh test DB first, then a user with
    // NEITHER sandbox perm.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `sbx_none_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@example.com`, 'password123', [])
    await login(page, baseURL, uname, 'password123')

    await gotoSandboxEnvironments(page, baseURL)

    await expect(
      page.getByText("You don't have permission to view sandbox environments."),
    ).toBeVisible({ timeout: 10000 })
  })
})
