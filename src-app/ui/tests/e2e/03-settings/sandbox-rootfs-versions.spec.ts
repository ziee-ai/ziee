import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

// ---------------------------------------------------------------------------
// Admin UI for the rootfs **version** lifecycle (Plan 5). Replaces the
// removed environments/prefetch admin spec. The real install endpoint
// downloads a ~74 MB squashfs + cosign-verifies it — slow + networked —
// so all /api/code-sandbox/rootfs/versions/* endpoints are mocked with
// deterministic responses. The SSE wire format matches the backend's
// committed SSEInstallTaskEvent union (camelCase event names).
// ---------------------------------------------------------------------------

const SSE_HEADERS = { contentType: 'text/event-stream' }

const ART_PINNED = {
  id: '00000000-0000-0000-0000-0000000000a3',
  version: '0.0.3',
  arch: 'x86_64',
  flavor: 'minimal',
  package: 'squashfs',
  sha256: 'a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3',
  artifact_path: '/cache/0.0.3/ziee-sandbox-rootfs-x86_64-minimal.squashfs',
  cosign_bundle: '/cache/0.0.3/ziee-sandbox-rootfs-x86_64-minimal.squashfs.cosign.bundle',
  status: 'installed',
  downloaded_at: '2026-05-30T00:00:00Z',
  last_used_at: null,
}
const ART_UNPINNED = {
  ...ART_PINNED,
  id: '00000000-0000-0000-0000-0000000000a4',
  version: '0.0.4',
  artifact_path: '/cache/0.0.4/ziee-sandbox-rootfs-x86_64-minimal.squashfs',
  cosign_bundle: '/cache/0.0.4/ziee-sandbox-rootfs-x86_64-minimal.squashfs.cosign.bundle',
}

function release(version: string) {
  return {
    version,
    published_at: '2026-05-30T00:00:00Z',
    draft: false,
    prerelease: false,
    asset_names: [],
  }
}

/** VersionStatus mock. `installed` defaults to the 0.0.3 (pinned) +
 *  0.0.4 (unpinned) minimal artifacts; `available` exposes both as
 *  GitHub releases so the synthetic full rows get a Download button. */
function versionStatus(over: Partial<Record<string, unknown>> = {}) {
  return {
    pinned_version: '0.0.3',
    installed: [ART_PINNED, ART_UNPINNED],
    available: [release('0.0.4'), release('0.0.3')],
    draining: [],
    conversation_count: 0,
    mcp_server_workspace_count: 0,
    last_swap: null,
    ...over,
  }
}

async function mockVersions(page: Page, get: () => unknown) {
  await page.route(/\/api\/code-sandbox\/rootfs\/versions$/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(get()),
    })
  })
}

/** The page subscribes to the install SSE on mount; return a stream
 *  that just emits `connected` so the store's subscriber is happy. */
async function mockInstallSse(page: Page) {
  await page.route(
    /\/api\/code-sandbox\/rootfs\/versions\/install\/subscribe$/,
    async route => {
      await route.fulfill({
        status: 200,
        ...SSE_HEADERS,
        body: 'event: connected\ndata: {"message":"connected"}\n\n',
      })
    },
  )
}

async function gotoSandbox(page: Page, baseURL: string) {
  // Vite can throw a transient 504 "Outdated Optimize Dep" mid-nav on a
  // cold cache; the only recovery is a reload. Retry until the heading
  // renders.
  const heading = page.getByRole('heading', { name: 'Code Sandbox' })
  for (let attempt = 1; attempt <= 3; attempt++) {
    await page.goto(`${baseURL}/settings/sandbox`)
    await page.waitForLoadState('networkidle').catch(() => {})
    try {
      await expect(heading).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

const minimalPinned = (page: Page) =>
  page.getByTestId('rootfs-row-0.0.3-minimal')
const minimalUnpinned = (page: Page) =>
  page.getByTestId('rootfs-row-0.0.4-minimal')
const fullAvailable = (page: Page) =>
  page.getByTestId('rootfs-row-0.0.4-full')

// ---------------------------------------------------------------------------

test.describe('Sandbox rootfs versions admin', () => {
  test.describe.configure({ retries: 2 })

  test('lists versions with pinned / downloaded / available status', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Header shows the current pin.
    await expect(page.getByTestId('pinned-chip')).toContainText('0.0.3')

    // The pinned, installed row carries the Pinned + Downloaded tags.
    // `exact` so we hit the status tag, not the "… · downloaded <date>"
    // sha256 caption line below it.
    await expect(minimalPinned(page).getByTestId('pinned-tag')).toBeVisible()
    await expect(
      minimalPinned(page).getByText('Downloaded', { exact: true }),
    ).toBeVisible()

    // The not-yet-downloaded `full` flavor offers a Download button.
    await expect(
      fullAvailable(page).getByRole('button', { name: 'Download' }),
    ).toBeVisible()
  })

  test('download click starts an install and shows progress', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    let started = false
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/install$/,
      async route => {
        started = true
        await route.fulfill({
          status: 202,
          contentType: 'application/json',
          body: JSON.stringify({
            task_id: '00000000-0000-0000-0000-0000000000ff',
            version: '0.0.4',
            arch: 'x86_64',
            flavor: 'full',
            package: 'squashfs',
            status: 'running',
            phase: 'downloading',
            message: 'downloading',
            started_at: '2026-05-30T00:00:00Z',
            completed_at: null,
            artifact_id: null,
            bytes_downloaded: null,
            duration_ms: null,
            error: null,
          }),
        })
      },
    )
    await gotoSandbox(page, baseURL)

    await fullAvailable(page).getByRole('button', { name: 'Download' }).click()
    expect(started).toBe(true)
    await expect(
      page.getByTestId('install-progress-0.0.4-full'),
    ).toBeVisible({ timeout: 10000 })
  })

  test('pin a downloaded version (non-major, no confirm)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    let pinnedTo = ''
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/set-pin$/,
      async route => {
        pinnedTo = (route.request().postDataJSON() as { version: string }).version
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            swap: { pinned: '0.0.4', was: '0.0.3', draining_mounts: 0, cache_wipe: 'preserve' },
            status: versionStatus({ pinned_version: '0.0.4' }),
          }),
        })
      },
    )
    await gotoSandbox(page, baseURL)

    // 0.0.3 → 0.0.4 is a patch bump (same major) → no confirm modal.
    await minimalUnpinned(page).getByRole('button', { name: 'Pin' }).click()
    await expect.poll(() => pinnedTo).toBe('0.0.4')
  })

  test('delete a downloaded non-pinned artifact', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    let deletedId = ''
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/[0-9a-f-]+$/,
      async route => {
        if (route.request().method() === 'DELETE') {
          deletedId = route.request().url().split('/').pop() ?? ''
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify(
              versionStatus({ installed: [ART_PINNED] }),
            ),
          })
        }
        return route.fallback()
      },
    )
    await gotoSandbox(page, baseURL)

    await minimalUnpinned(page).getByTestId('rootfs-delete-button').click()
    // Popconfirm OK button text is "Delete" — scope to the popover.
    await page
      .locator('.ant-popconfirm, .ant-popover')
      .getByRole('button', { name: 'Delete' })
      .click()
    await expect.poll(() => deletedId).toBe(ART_UNPINNED.id)
  })

  test('refresh re-fetches the version list', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    let calls = 0
    await mockVersions(page, () => {
      calls += 1
      return versionStatus()
    })
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)
    const before = calls
    await page.getByTestId('rootfs-refresh-button').click()
    await expect.poll(() => calls).toBeGreaterThan(before)
  })

  test('manage permission gates the action buttons', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `rootfs_read_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@example.com`, 'password123', [
      'code_sandbox::environments::read',
    ])
    await login(page, baseURL, uname, 'password123')

    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Read-only: rows render, but Download / Pin / Delete are disabled.
    await expect(
      fullAvailable(page).getByRole('button', { name: 'Download' }),
    ).toBeDisabled()
    await expect(
      minimalUnpinned(page).getByRole('button', { name: 'Pin' }),
    ).toBeDisabled()
    await expect(
      minimalUnpinned(page).getByTestId('rootfs-delete-button'),
    ).toBeDisabled()
  })

  test('read permission gates the whole page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `rootfs_none_${Date.now()}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@example.com`, 'password123', [])
    await login(page, baseURL, uname, 'password123')

    await page.goto(`${baseURL}/settings/sandbox`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible({ timeout: 10000 })
    expect(page.url()).toContain('/settings/sandbox')
  })
})
