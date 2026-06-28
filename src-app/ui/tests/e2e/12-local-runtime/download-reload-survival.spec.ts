import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * E2E — page-reload survival of an in-progress runtime-engine download
 * (audit gap all-6c42f9238673).
 *
 * Backend engine downloads are detached (`tokio::spawn` in `download_task`),
 * so a page reload doesn't cancel them. The reload-survival guarantee is a
 * CLIENT property: on a fresh mount the `RuntimeDownloadProgress` store's
 * `__init__` runs `loadActive()` — a real `GET /local-runtime/versions/downloads`
 * — and repaints the in-flight progress bar without any carried-over JS state
 * (RuntimeDownloadProgress.store.ts:62-85). No prior spec exercised this:
 * `04-engine-lifecycle` only runs against a real engine+network and never
 * reloads mid-download.
 *
 * The detached-download server registry is in-memory (a DashMap keyed by
 * engine@version@backend), so an in-flight task can only exist behind a real
 * running download — not seedable via the DB. To exercise the re-hydration
 * deterministically and offline we mock ONLY the two server-data boundaries the
 * page reads — the upstream release-feed check and the active-downloads list —
 * and let the behavior under test run for real: the store's mount-time
 * `loadActive()` re-fetch, the snapshot→`activeByKey` mapping, and the
 * `AvailableVersionsCard` progress-bar render. The page reload wipes ALL JS
 * state, so the progress bar can only reappear via a fresh `loadActive()` fetch
 * — which is exactly the reload-survival property. (Mocking the data endpoints
 * is the external boundary; the re-hydration + render are never mocked.)
 */

const ENGINE = 'llamacpp'
const VERSION = '9.9.9-reload-test'
const BACKEND = 'cpu'
const KEY = `${ENGINE}@${VERSION}@${BACKEND}`

const UPDATE_CHECK_BODY = {
  engine: ENGINE,
  platform: 'linux',
  arch: 'x86_64',
  versions: [
    {
      version: VERSION,
      installed: false,
      installed_backends: [],
      binary_ready: true,
      available_backends: [BACKEND],
      recommended_backend: BACKEND,
      size_bytes: 117_440_512,
      prerelease: false,
    },
  ],
}

// A non-terminal ("downloading") snapshot, ~40% through.
const IN_FLIGHT_SNAPSHOT = {
  task_id: 'reload-survival-task',
  key: KEY,
  engine: ENGINE,
  version: VERSION,
  backend: BACKEND,
  status: 'downloading',
  bytes_received: 47_000_000,
  total_bytes: 117_440_512,
  percent: 40,
}

test.describe('Local Runtime — in-progress download survives a page reload', () => {
  test('reload re-hydrates the active download via loadActive() and repaints its progress bar', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Count fresh fetches of the active-downloads list so we can prove the
    // post-reload progress bar came from a NEW server fetch (re-hydration),
    // not stale in-memory state (which the reload destroys).
    let downloadsListFetches = 0

    // (1) Upstream release-feed check — yields one ready, not-installed
    // version so AvailableVersionsCard renders a row whose
    // engine@version@backend key matches the in-flight snapshot.
    await page.route(
      /\/api\/local-runtime\/versions\/[^/]+\/check-updates(\?|$)/,
      async route => {
        const engine = new URL(route.request().url()).pathname.split('/').slice(-2)[0]
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          // Only llamacpp gets the in-flight version; other engines stay empty.
          body: JSON.stringify(
            engine === ENGINE
              ? UPDATE_CHECK_BODY
              : { engine, platform: 'linux', arch: 'x86_64', versions: [] },
          ),
        })
      },
    )

    // (2) Active-downloads list — the loadActive() boundary. Matches ONLY the
    // collection endpoint (not /{key} nor /{key}/events).
    await page.route(
      /\/api\/local-runtime\/versions\/downloads(\?|$)/,
      async route => {
        downloadsListFetches += 1
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ downloads: [IN_FLIGHT_SNAPSHOT] }),
        })
      },
    )

    // (3) The per-key SSE stream the store opens for non-terminal tasks —
    // close it immediately so the test doesn't hang on a live subscription.
    // (We assert re-hydration via loadActive, not via live SSE chunks.)
    await page.route(
      /\/api\/local-runtime\/versions\/downloads\/[^/]+\/events(\?|$)/,
      async route => {
        await route.fulfill({
          status: 200,
          contentType: 'text/event-stream',
          body: '',
        })
      },
    )

    await gotoRuntimeSettings(page, baseURL)

    const pane = page.locator('.ant-tabs-tabpane-active')
    await expect(pane.getByText(/Available versions/i).first()).toBeVisible({
      timeout: 30_000,
    })

    // The mocked available version row renders (its Install button carries the
    // aria-label `Install <version>`), and the in-flight snapshot paints a
    // progress bar at 40% under it.
    await expect(
      pane.getByRole('button', { name: `Install ${VERSION}` }),
    ).toBeVisible({ timeout: 30_000 })
    await expect(pane.locator('.ant-progress').first()).toBeVisible({
      timeout: 30_000,
    })
    await expect(pane.getByText('40%').first()).toBeVisible({ timeout: 30_000 })

    expect(
      downloadsListFetches,
      'loadActive() must fetch the active-downloads list on first mount',
    ).toBeGreaterThanOrEqual(1)
    const fetchesBeforeReload = downloadsListFetches

    // --- The reload: destroys all JS state (store maps, SSE controllers). ---
    await page.reload()
    await expect(page).toHaveURL(/\/settings\/llm-runtime$/)

    const paneAfter = page.locator('.ant-tabs-tabpane-active')
    await expect(paneAfter.getByText(/Available versions/i).first()).toBeVisible({
      timeout: 30_000,
    })

    // The progress bar reappears AFTER the reload — it can only have come from
    // a fresh loadActive() re-fetch, since the reload wiped the in-memory map.
    await expect(
      paneAfter.getByRole('button', { name: `Install ${VERSION}` }),
    ).toBeVisible({ timeout: 30_000 })
    await expect(paneAfter.locator('.ant-progress').first()).toBeVisible({
      timeout: 30_000,
    })
    await expect(paneAfter.getByText('40%').first()).toBeVisible({
      timeout: 30_000,
    })

    expect(
      downloadsListFetches,
      'reload must trigger a NEW loadActive() fetch (re-hydration, not stale state)',
    ).toBeGreaterThan(fetchesBeforeReload)
  })
})
