import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

// ---------------------------------------------------------------------------
// Admin UI for the rootfs **version** lifecycle (version-grouped two-card
// redesign). The page splits versions into a "Downloaded versions" card and an
// "Available versions" card, grouped by version with flavors nested underneath.
// Download is atomic over flavors: one per-version Download button fetches every
// missing host-arch flavor at once. "Set as Default" == the backend pin (with a
// major-version-bump confirm). The real install endpoint downloads a large
// squashfs + cosign-verifies it, so every /api/code-sandbox/rootfs/versions/*
// endpoint is mocked. The SSE wire format matches the backend's committed
// SSEInstallTaskEvent union (camelCase event names).
// ---------------------------------------------------------------------------

const SSE_HEADERS = { contentType: 'text/event-stream' }

/** Release asset names → exercises `parseAssetName` / flavor grouping. */
function assetNames(arch = 'x86_64', pkg: 'squashfs' | 'tar.zst' = 'squashfs') {
  return [
    `ziee-sandbox-rootfs-${arch}-minimal.${pkg}`,
    `ziee-sandbox-rootfs-${arch}-full.${pkg}`,
  ]
}

function art(
  version: string,
  flavor: string,
  idTail: string,
  arch = 'x86_64',
  pkg: 'squashfs' | 'tar.zst' = 'squashfs',
) {
  return {
    id: `00000000-0000-0000-0000-0000000000${idTail}`,
    version,
    arch,
    flavor,
    package: pkg,
    sha256: `${idTail}`.padEnd(64, idTail[0] ?? 'a'),
    artifact_path: `/cache/${version}/ziee-sandbox-rootfs-${arch}-${flavor}.${pkg}`,
    cosign_bundle: `/cache/${version}/ziee-sandbox-rootfs-${arch}-${flavor}.${pkg}.cosign.bundle`,
    status: 'installed',
    downloaded_at: '2026-05-30T00:00:00Z',
    last_used_at: null,
  }
}

/** Asset names for a release that publishes BOTH packages per flavor. */
function bothPackageAssets(arch = 'x86_64') {
  return [...assetNames(arch, 'squashfs'), ...assetNames(arch, 'tar.zst')]
}

interface ReleaseOpts {
  arch?: string
  pkg?: 'squashfs' | 'tar.zst'
  draft?: boolean
  prerelease?: boolean
}
function release(version: string, opts: ReleaseOpts = {}) {
  return {
    version,
    published_at: '2026-05-30T00:00:00Z',
    draft: opts.draft ?? false,
    prerelease: opts.prerelease ?? false,
    asset_names: assetNames(opts.arch ?? 'x86_64', opts.pkg ?? 'squashfs'),
  }
}

// Fixture matrix:
//   0.0.3 — DEFAULT, fully downloaded (minimal+full) → Downloaded card.
//   0.0.5 — fully downloaded, NOT default            → Downloaded card.
//   1.0.0 — fully downloaded, NOT default            → Downloaded card (major bump).
//   0.0.4 — nothing downloaded, catalog minimal+full → Available card.
function versionStatus(over: Partial<Record<string, unknown>> = {}) {
  return {
    pinned_version: '0.0.3',
    installed: [
      art('0.0.3', 'minimal', '31'),
      art('0.0.3', 'full', '32'),
      art('0.0.5', 'minimal', '51'),
      art('0.0.5', 'full', '52'),
      art('1.0.0', 'minimal', 'a1'),
      art('1.0.0', 'full', 'a2'),
    ],
    available: [release('1.0.0'), release('0.0.5'), release('0.0.4'), release('0.0.3')],
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

/** The page subscribes to the install SSE on mount; emit `connected` plus any
 *  extra pre-scripted SSE frames (e.g. a replayed taskState). */
async function mockInstallSse(page: Page, extraFrames = '') {
  // Emit the scripted frames only on the FIRST connect; a reconnect (the
  // finite body closes the stream) gets `connected` only, so it can't re-add a
  // task the test has since pruned via Refresh.
  let sent = false
  await page.route(
    /\/api\/code-sandbox\/rootfs\/versions\/install\/subscribe$/,
    async route => {
      const frames = sent ? '' : extraFrames
      sent = true
      await route.fulfill({
        status: 200,
        ...SSE_HEADERS,
        body: 'event: connected\ndata: {"message":"connected"}\n\n' + frames,
      })
    },
  )
}

/** Build a single SSE frame for the install-progress stream. */
function sseTaskFrame(event: string, task: Record<string, unknown>) {
  return `event: ${event}\ndata: ${JSON.stringify(task)}\n\n`
}

interface InstallReq {
  version: string
  arch: string
  flavor: string
  package: string
}
/** Capture every install POST and 202 a running task. Returns the capture array. */
async function captureInstalls(page: Page): Promise<InstallReq[]> {
  const installs: InstallReq[] = []
  await page.route(
    /\/api\/code-sandbox\/rootfs\/versions\/install$/,
    async route => {
      const b = route.request().postDataJSON() as InstallReq
      installs.push(b)
      await route.fulfill({
        status: 202,
        contentType: 'application/json',
        body: JSON.stringify({
          task_id: `task-${b.version}-${b.flavor}`,
          version: b.version,
          arch: b.arch,
          flavor: b.flavor,
          package: b.package,
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
  return installs
}

async function gotoSandbox(page: Page, baseURL: string) {
  const heading = page.getByRole('heading', { name: 'Code Sandbox' })
  for (let attempt = 1; attempt <= 3; attempt++) {
    await page.goto(`${baseURL}/settings/sandbox`)
    await page.waitForLoadState('load').catch(() => {})
    try {
      await expect(heading).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

const downloadedCard = (page: Page) => page.getByTestId('downloaded-versions-card')
const availableCard = (page: Page) => page.getByTestId('available-versions-card')
const group = (page: Page, v: string) => page.getByTestId(`rootfs-version-group-${v}`)
// Card-scoped group locators so action tests also assert the version is in the
// EXPECTED card (the partition is part of the behavior under test).
const dlGroup = (page: Page, v: string) =>
  downloadedCard(page).getByTestId(`rootfs-version-group-${v}`)
const availGroup = (page: Page, v: string) =>
  availableCard(page).getByTestId(`rootfs-version-group-${v}`)

// ---------------------------------------------------------------------------

test.describe('Sandbox rootfs versions admin', () => {
  test.describe.configure({ retries: 2 })

  test('renders the two cards with versions grouped + flavors nested', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Both cards present.
    await expect(downloadedCard(page)).toBeVisible()
    await expect(availableCard(page)).toBeVisible()

    // Header shows the current default + the downloaded-flavors summary.
    await expect(page.getByTestId('default-chip')).toContainText('0.0.3')
    await expect(page.getByTestId('downloaded-flavors')).toContainText('x86_64-minimal')

    // 0.0.3 is fully downloaded → Downloaded card, with minimal + full nested.
    await expect(dlGroup(page, '0.0.3')).toBeVisible()
    await expect(
      dlGroup(page, '0.0.3').getByTestId('rootfs-row-0.0.3-minimal'),
    ).toBeVisible()
    await expect(
      dlGroup(page, '0.0.3').getByTestId('rootfs-row-0.0.3-full'),
    ).toBeVisible()
    // It's the default → "Default" tag, and no Set-as-Default / Delete buttons.
    await expect(dlGroup(page, '0.0.3').getByTestId('default-tag')).toBeVisible()
    await expect(
      dlGroup(page, '0.0.3').getByRole('button', { name: 'Set as Default' }),
    ).toHaveCount(0)

    // 0.0.4 (nothing downloaded) → Available card, with a Download button.
    await expect(availGroup(page, '0.0.4')).toBeVisible()
    await expect(
      availGroup(page, '0.0.4').getByRole('button', { name: 'Download' }),
    ).toBeVisible()
  })

  test('the DEFAULT version cannot be deleted (no delete button); non-default can', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // 0.0.3 is the current default → it carries the default tag and exposes
    // NEITHER a "Set as Default" nor a Delete control (RootfsVersionGroup gates
    // both on `!group.isDefault`). This guards the "can't delete the default"
    // invariant the prior test only half-asserted (it checked Set-as-Default).
    await expect(dlGroup(page, '0.0.3').getByTestId('default-tag')).toBeVisible()
    await expect(page.getByTestId('rootfs-delete-0.0.3')).toHaveCount(0)

    // A non-default downloaded version DOES expose the delete control
    // (positive control so the assertion above isn't vacuously true).
    await expect(page.getByTestId('rootfs-delete-0.0.5')).toBeVisible()
  })

  test('Download fetches ALL flavors of a version at once', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    await availGroup(page, '0.0.4').getByRole('button', { name: 'Download' }).click()

    // Both flavors of 0.0.4 are requested (atomic over flavors).
    await expect
      .poll(() => installed.filter(i => i.version === '0.0.4').length)
      .toBe(2)
    expect(
      installed
        .filter(i => i.version === '0.0.4')
        .map(i => i.flavor)
        .sort(),
    ).toEqual(['full', 'minimal'])

    // Version-level aggregate progress bar appears.
    await expect(page.getByTestId('install-progress-0.0.4')).toBeVisible({
      timeout: 10000,
    })
  })

  test('Download of a partially-downloaded version fetches only the missing flavor', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // 0.0.6 has ONLY minimal downloaded; catalog offers minimal+full.
    await mockVersions(page, () =>
      versionStatus({
        installed: [
          art('0.0.3', 'minimal', '31'),
          art('0.0.3', 'full', '32'),
          art('0.0.6', 'minimal', '61'),
        ],
        available: [release('0.0.6'), release('0.0.3')],
      }),
    )
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    // Partial version lives in the Available card (not fully downloaded), with
    // minimal already "Downloaded" and full still "Available".
    await expect(availGroup(page, '0.0.6')).toBeVisible()
    await expect(downloadedCard(page).getByTestId('rootfs-version-group-0.0.6')).toHaveCount(0)
    await expect(
      availGroup(page, '0.0.6')
        .getByTestId('rootfs-row-0.0.6-minimal')
        .getByText('Downloaded', { exact: true }),
    ).toBeVisible()
    await expect(
      availGroup(page, '0.0.6')
        .getByTestId('rootfs-row-0.0.6-full')
        .getByText('Available', { exact: true }),
    ).toBeVisible()

    await availGroup(page, '0.0.6').getByRole('button', { name: 'Download' }).click()
    // Only the missing flavor (full) is installed — NOT minimal again.
    await expect.poll(() => installed.length).toBe(1)
    expect(installed[0]).toMatchObject({ version: '0.0.6', flavor: 'full' })
  })

  test('Set as Default on a same-major version (no confirm modal)', async ({
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
            swap: {
              pinned: '0.0.5',
              was: '0.0.3',
              draining_mounts: 0,
              cache_wipe: 'preserve',
            },
            status: versionStatus({ pinned_version: '0.0.5' }),
          }),
        })
      },
    )
    await gotoSandbox(page, baseURL)

    // 0.0.3 → 0.0.5 is a patch bump (same major 0) → no confirm modal.
    await dlGroup(page, '0.0.5')
      .getByRole('button', { name: 'Set as Default' })
      .click()
    await expect.poll(() => pinnedTo).toBe('0.0.5')
    await expect(page.getByText(/major version bump/i)).toHaveCount(0)
  })

  test('Set as Default across a MAJOR bump shows a confirm + wipe warning', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () =>
      versionStatus({ conversation_count: 3, mcp_server_workspace_count: 1 }),
    )
    await mockInstallSse(page)
    let pinnedTo = ''
    let pinCalls = 0
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/set-pin$/,
      async route => {
        pinCalls += 1
        pinnedTo = (route.request().postDataJSON() as { version: string }).version
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            swap: {
              pinned: '1.0.0',
              was: '0.0.3',
              draining_mounts: 2,
              cache_wipe: 'wipe_caches_on_drain',
            },
            status: versionStatus({ pinned_version: '1.0.0' }),
          }),
        })
      },
    )
    await gotoSandbox(page, baseURL)

    // First: open the modal and CANCEL — no set-pin call should fire.
    await dlGroup(page, '1.0.0')
      .getByRole('button', { name: 'Set as Default' })
      .click()
    // antd v6 renders the confirm title twice — a visually-hidden
    // .ant-modal-title (the dialog's aria-label) + the visible
    // .ant-modal-confirm-title. Match the dialog by its accessible name rather
    // than a title text node (the hidden one fails toBeVisible).
    const modal = page.getByRole('dialog', { name: /major version bump/i })
    await expect(modal).toBeVisible()
    await expect(modal.getByText(/3 conversation workspaces/i)).toBeVisible()
    await expect(modal.getByText(/1 sandboxed MCP server workspace/i)).toBeVisible()
    await modal.getByRole('button', { name: 'Cancel' }).click()
    await page.waitForTimeout(300)
    expect(pinCalls).toBe(0)

    // Then: open again and CONFIRM → set-pin to 1.0.0.
    await dlGroup(page, '1.0.0')
      .getByRole('button', { name: 'Set as Default' })
      .click()
    await page
      .getByRole('dialog')
      .getByRole('button', { name: /Set as default and wipe caches/i })
      .click()
    await expect.poll(() => pinnedTo).toBe('1.0.0')

    // The swap returned in-flight sessions → draining indicator surfaces.
    await expect(page.getByTestId('draining-indicator')).toBeVisible()
  })

  test('per-row draining + in-flight tags render from the draining list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // 0.0.5 (non-default) is still serving in-flight sessions on the old mount.
    await mockVersions(page, () =>
      versionStatus({
        draining: [
          {
            version: '0.0.5',
            arch: 'x86_64',
            flavor: 'minimal',
            artifact_id: '00000000-0000-0000-0000-000000000051',
            inflight_exec: 2,
            inflight_mcp: 1,
          },
        ],
      }),
    )
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    await expect(dlGroup(page, '0.0.5').getByTestId('row-draining')).toBeVisible()
    await expect(
      page.getByTestId('inflight-0.0.5-minimal'),
    ).toContainText('2 exec')
    await expect(page.getByTestId('inflight-0.0.5-minimal')).toContainText('1 MCP')
  })

  test('Delete removes every flavor of a downloaded version and it leaves the card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const deleted: string[] = []
    // Stateful: the status response drops the deleted artifacts so the version
    // actually disappears from the Downloaded card.
    await mockVersions(page, () =>
      versionStatus({
        installed: versionStatus().installed.filter(
          (a: { id: string }) => !deleted.includes(a.id),
        ),
      }),
    )
    await mockInstallSse(page)
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/[0-9a-f-]+$/,
      async route => {
        if (route.request().method() === 'DELETE') {
          const id = route.request().url().split('/').pop() ?? ''
          deleted.push(id)
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify(
              versionStatus({
                installed: versionStatus().installed.filter(
                  (a: { id: string }) => !deleted.includes(a.id),
                ),
              }),
            ),
          })
        }
        return route.fallback()
      },
    )
    await gotoSandbox(page, baseURL)

    await dlGroup(page, '0.0.5').getByRole('button', { name: 'Delete' }).click()
    await page
      .locator('.ant-popconfirm, .ant-popover')
      .getByRole('button', { name: 'Delete' })
      .click()

    // Both 0.0.5 artifacts (minimal id ...51, full id ...52) are deleted...
    await expect.poll(() => deleted.length).toBe(2)
    expect(deleted.sort()).toEqual([
      '00000000-0000-0000-0000-000000000051',
      '00000000-0000-0000-0000-000000000052',
    ])
    // ...and the version is gone from the Downloaded card. Refresh first so the
    // final state is deterministic (the mockVersions closure now filters BOTH
    // ids) rather than depending on which concurrent DELETE response resolved
    // last.
    await page.getByTestId('rootfs-refresh-button').click()
    await expect(
      downloadedCard(page).getByTestId('rootfs-version-group-0.0.5'),
    ).toHaveCount(0)
  })

  test('skips draft/prerelease releases and parses tar.zst assets', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () =>
      versionStatus({
        installed: [art('0.0.3', 'minimal', '31'), art('0.0.3', 'full', '32')],
        available: [
          release('0.2.2', { pkg: 'tar.zst' }),
          release('0.1.1', { prerelease: true }),
          release('0.0.9', { draft: true }),
          release('0.0.3'),
        ],
      }),
    )
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    // Draft + prerelease releases never surface.
    await expect(group(page, '0.0.9')).toHaveCount(0)
    await expect(group(page, '0.1.1')).toHaveCount(0)
    // The tar.zst release parses + offers a Download.
    await expect(availGroup(page, '0.2.2')).toBeVisible()
    await availGroup(page, '0.2.2').getByRole('button', { name: 'Download' }).click()
    await expect.poll(() => installed.filter(i => i.version === '0.2.2').length).toBe(2)
    // parseAssetName resolved the .tar.zst suffix → package routed through.
    expect(installed.filter(i => i.version === '0.2.2').every(i => i.package === 'tar.zst')).toBe(true)
  })

  test('a release shipping both squashfs + tar.zst renders each flavor once', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Real GitHub releases publish BOTH a .squashfs (Linux/macOS) and a
    // .tar.zst (Windows) asset per flavor. Host package here is squashfs
    // (derived from the installed 0.0.3 artifacts).
    await mockVersions(page, () =>
      versionStatus({
        installed: [art('0.0.3', 'minimal', '31'), art('0.0.3', 'full', '32')],
        available: [
          {
            version: '0.7.0',
            published_at: '2026-05-30T00:00:00Z',
            draft: false,
            prerelease: false,
            asset_names: [
              'ziee-sandbox-rootfs-x86_64-minimal.squashfs',
              'ziee-sandbox-rootfs-x86_64-full.squashfs',
              'ziee-sandbox-rootfs-x86_64-minimal.tar.zst',
              'ziee-sandbox-rootfs-x86_64-full.tar.zst',
            ],
          },
          release('0.0.3'),
        ],
      }),
    )
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    // Each flavor sub-row appears EXACTLY ONCE (regression: both-package
    // releases used to render `minimal`/`full` twice — one per package).
    await expect(availGroup(page, '0.7.0')).toBeVisible()
    await expect(
      availGroup(page, '0.7.0').getByTestId('rootfs-row-0.7.0-minimal'),
    ).toHaveCount(1)
    await expect(
      availGroup(page, '0.7.0').getByTestId('rootfs-row-0.7.0-full'),
    ).toHaveCount(1)

    // The host package (squashfs) is chosen for the Download, not tar.zst.
    await availGroup(page, '0.7.0').getByRole('button', { name: 'Download' }).click()
    await expect.poll(() => installed.filter(i => i.version === '0.7.0').length).toBe(2)
    expect(
      installed.filter(i => i.version === '0.7.0').every(i => i.package === 'squashfs'),
    ).toBe(true)
  })

  test('a flavor downloaded in a non-host package still renders once and is not re-offered', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // 0.9.0 is fully downloaded but with MIXED packages — minimal as tar.zst,
    // full as squashfs — while the host package derives to squashfs (majority)
    // and the catalog ships both packages per flavor. Package-agnostic grouping
    // must still collapse each flavor to ONE row and treat it as downloaded.
    await mockVersions(page, () =>
      versionStatus({
        installed: [
          art('0.0.3', 'minimal', '31'),
          art('0.0.3', 'full', '32'),
          art('0.9.0', 'minimal', '91', 'x86_64', 'tar.zst'),
          art('0.9.0', 'full', '92', 'x86_64', 'squashfs'),
        ],
        available: [
          {
            version: '0.9.0',
            published_at: '2026-05-30T00:00:00Z',
            draft: false,
            prerelease: false,
            asset_names: bothPackageAssets(),
          },
          release('0.0.3'),
        ],
      }),
    )
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Fully downloaded → Downloaded card, NOT re-offered in Available, and each
    // flavor renders exactly once despite the package divergence.
    await expect(dlGroup(page, '0.9.0')).toBeVisible()
    await expect(
      availableCard(page).getByTestId('rootfs-version-group-0.9.0'),
    ).toHaveCount(0)
    await expect(
      dlGroup(page, '0.9.0').getByTestId('rootfs-row-0.9.0-minimal'),
    ).toHaveCount(1)
    await expect(
      dlGroup(page, '0.9.0').getByTestId('rootfs-row-0.9.0-full'),
    ).toHaveCount(1)
  })

  test('uses the server host package on a fresh host (Windows → tar.zst)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Fresh host (nothing installed) where the server reports a Windows/WSL2
    // host — the UI must offer tar.zst (what the backend can mount), NOT the
    // squashfs client-default.
    await mockVersions(page, () =>
      versionStatus({
        pinned_version: null,
        installed: [],
        host_arch: 'x86_64',
        host_package: 'tar.zst',
        available: [
          {
            version: '0.0.4',
            published_at: '2026-05-30T00:00:00Z',
            draft: false,
            prerelease: false,
            asset_names: bothPackageAssets(),
          },
        ],
      }),
    )
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    await availGroup(page, '0.0.4').getByRole('button', { name: 'Download' }).click()
    await expect.poll(() => installed.filter(i => i.version === '0.0.4').length).toBe(2)
    // Server-authoritative host package (tar.zst) chosen, not squashfs default.
    expect(
      installed.filter(i => i.version === '0.0.4').every(i => i.package === 'tar.zst'),
    ).toBe(true)
  })

  test('a failed install survives a sibling completion and only clears on Refresh', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())
    // On SSE connect, replay a terminal-FAILED taskState for 0.0.4-minimal AND a
    // `complete` for a sibling — the `complete` handler auto-fires loadStatus(),
    // which must NOT prune the still-failed flavor's bar (round-4 regression).
    await mockInstallSse(
      page,
      sseTaskFrame('taskState', {
        task_id: 't-0.0.4-min',
        version: '0.0.4',
        arch: 'x86_64',
        flavor: 'minimal',
        package: 'squashfs',
        status: 'failed',
        phase: 'failed',
        message: 'boom',
        started_at: '2026-05-30T00:00:00Z',
        completed_at: null,
        artifact_id: null,
        bytes_downloaded: null,
        duration_ms: null,
        error: 'disk full',
      }) +
        sseTaskFrame('complete', {
          task_id: 't-sibling-complete',
          artifact_id: '00000000-0000-0000-0000-0000000000ff',
          bytes_downloaded: 1,
          cosign_verified: true,
          duration_ms: 1,
        }),
    )
    await gotoSandbox(page, baseURL)

    // The failed flavor surfaces a (red exception) progress bar...
    await expect(page.getByTestId('install-progress-0.0.4')).toBeVisible({
      timeout: 10000,
    })
    // ...and SURVIVES the sibling `complete`'s auto-loadStatus (no pruneFailed).
    await page.waitForTimeout(500)
    await expect(page.getByTestId('install-progress-0.0.4')).toBeVisible()
    // Only an EXPLICIT Refresh (pruneFailed) clears the stuck failed bar.
    await page.getByTestId('rootfs-refresh-button').click()
    await expect(page.getByTestId('install-progress-0.0.4')).toHaveCount(0)
  })

  test('derives host arch from installed artifacts (aarch64)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Majority (here: all) installed artifacts are aarch64 → hostArch=aarch64.
    await mockVersions(page, () =>
      versionStatus({
        pinned_version: '0.0.8',
        installed: [
          art('0.0.8', 'minimal', '81', 'aarch64'),
          art('0.0.8', 'full', '82', 'aarch64'),
        ],
        available: [
          release('0.0.8', { arch: 'aarch64' }),
          release('0.0.7', { arch: 'aarch64' }),
        ],
      }),
    )
    await mockInstallSse(page)
    const installed = await captureInstalls(page)
    await gotoSandbox(page, baseURL)

    await availGroup(page, '0.0.7').getByRole('button', { name: 'Download' }).click()
    await expect.poll(() => installed.filter(i => i.version === '0.0.7').length).toBe(2)
    // The host-arch filter offered aarch64 flavors, not the x86_64 default.
    expect(installed.filter(i => i.version === '0.0.7').every(i => i.arch === 'aarch64')).toBe(true)
  })

  test('empty available catalog shows the Empty guidance', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus({ available: [] }))
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Downloaded versions still render (installed is unchanged).
    await expect(dlGroup(page, '0.0.3')).toBeVisible()
    // Available card shows the GitHub-unreachable guidance (no enabled hint).
    await expect(
      availableCard(page).getByText(/No versions available to download/i),
    ).toBeVisible()
    await expect(
      availableCard(page).getByText(/api\.github\.com/i),
    ).toBeVisible()
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

    // Read-only: groups render, but Download / Set-as-Default / Delete disabled.
    await expect(
      availGroup(page, '0.0.4').getByRole('button', { name: 'Download' }),
    ).toBeDisabled()
    await expect(
      dlGroup(page, '0.0.5').getByRole('button', { name: 'Set as Default' }),
    ).toBeDisabled()
    await expect(
      dlGroup(page, '0.0.5').getByRole('button', { name: 'Delete' }),
    ).toBeDisabled()
  })

  test('resource-limits-only admin sees the rootfs section denial but reaches the page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `rootfs_rl_${Date.now()}`
    // resource_limits::read admits the route (anyOf) but NOT the rootfs section.
    await createTestUser(apiURL, adminToken, uname, `${uname}@example.com`, 'password123', [
      'code_sandbox::resource_limits::read',
    ])
    await login(page, baseURL, uname, 'password123')

    await mockVersions(page, () => versionStatus())
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    // Page rendered (route admitted), but the rootfs section shows its own
    // gate rather than the version cards.
    await expect(
      page.getByText(/don't have permission to view rootfs versions/i),
    ).toBeVisible()
    await expect(downloadedCard(page)).toHaveCount(0)
  })

  test('SSE subscribe failure surfaces a reconnecting error + retries', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockVersions(page, () => versionStatus())

    // The install-progress SSE endpoint hard-fails (503). The store's catch
    // sets `error = "SSE disconnected; reconnecting (attempt n/5)"` (rendered
    // as a top-level error Alert) and schedules a bounded reconnect.
    let subscribeCalls = 0
    await page.route(
      /\/api\/code-sandbox\/rootfs\/versions\/install\/subscribe$/,
      async route => {
        subscribeCalls += 1
        await route.fulfill({
          status: 503,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'unavailable' }),
        })
      },
    )

    await gotoSandbox(page, baseURL)

    // The disconnect error surfaces.
    await expect(
      page.getByText(/SSE disconnected; reconnecting/),
    ).toBeVisible({ timeout: 15000 })

    // A bounded reconnect fires (delay is 3s) — the store re-subscribes.
    await expect.poll(() => subscribeCalls, { timeout: 15000 }).toBeGreaterThan(1)
  })

  test('Downloaded card shows the empty state when nothing is downloaded', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    // Nothing installed → the Downloaded card renders its <Empty> placeholder;
    // every release lands in the Available card instead.
    await mockVersions(page, () =>
      versionStatus({ pinned_version: null, installed: [] }),
    )
    await mockInstallSse(page)
    await gotoSandbox(page, baseURL)

    await expect(downloadedCard(page)).toBeVisible()
    await expect(
      downloadedCard(page).getByText(
        'No rootfs versions downloaded yet. Download one from the Available versions list below.',
      ),
    ).toBeVisible()
    // The Available card still lists a downloadable release.
    await expect(
      availGroup(page, '0.0.4').getByRole('button', { name: 'Download' }),
    ).toBeVisible()
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
