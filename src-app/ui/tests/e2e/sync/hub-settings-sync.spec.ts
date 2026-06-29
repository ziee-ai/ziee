import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// Realtime sync for the `hub_settings` singleton entity. When an admin
// activates a different hub catalog version, the backend publishes
// `HubSettings/Update` to every `hub::catalog::read` holder so other
// devices' `useHubCatalogStore` refetches the version + reloads every
// hub tab WITHOUT a manual reload.
//
// This spec hits the REAL `ziee-ai/hub` GitHub repository. The
// activate handler always calls `HubManager::refresh()`, which:
//   1. GETs `https://api.github.com/repos/ziee-ai/hub/releases`
//      to list versions (unauthenticated — 60/hr per IP).
//   2. Downloads the requested release tarball + index + cosign
//      bundle (a few hundred KB).
//   3. Sigstore-verifies the bundle in-process (no network at this step).
//   4. Rotates the on-disk `current/` symlink to the new dir.
// Then `sync_publish` fires unconditionally.
//
// The bundled seed version (built into the test backend) is
// `0.0.3-alpha` (per `binaries/hub-seed/.tag`). The repo currently has
// three releases — `v0.0.{1,2,3}-alpha`. The test activates `0.0.2-alpha`
// and asserts device B's VersionPicker tag updates from `v0.0.3-alpha`
// to `v0.0.2-alpha`. If the repo ever publishes a `v0.0.3-alpha` rename
// or removes the older releases, update the version pin below.
//
// Run with --workers=1.
//
// Network / rate-limit notes:
//   - This test consumes one `releases` API call + one tarball download
//     per run. Far below the 60/hr unauthenticated GitHub limit for
//     local dev. If the test starts flaking with "API rate limit
//     exceeded" on a shared CI IP, the runtime path would need to
//     start honoring `GITHUB_TOKEN` (it currently doesn't — the build
//     helper does, but `fetch_releases` in hub_manager.rs uses no auth).
//   - Cosign verification + tarball roundtrip can take 10–20s; the
//     assertion timeout is set generously (45s) to absorb that without
//     becoming a Heisen-flake on a slow link.

// We pin a SPECIFIC alternate version (not "any version != current") so
// the test can't accidentally activate the same seed version (no-op
// emit, no visible change). 0.0.2-alpha is the second-newest published
// release and has been stable since the v0.0.3 cut.
const TARGET_VERSION = '0.0.2-alpha'

async function gotoHub(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/hub`)
  await page.waitForLoadState('load')
  // The VersionPicker tag is the stable signal that the admin Hub
  // page (and the HubCatalog store subscribing to sync:hub_settings)
  // has mounted.
  await expect(
    byTestId(page, 'hub-version-tag'),
  ).toBeVisible({ timeout: 30_000 })
}

function versionTag(page: import('@playwright/test').Page) {
  // The installed-catalog version renders as `<Tag>v{hubVersion}</Tag>` with
  // the hub-version-tag testid on the Hub page header.
  return byTestId(page, 'hub-version-tag')
}

test.describe('Realtime sync — hub_settings (real GitHub)', () => {
  test('activating a different catalog version on device A updates device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await gotoHub(page, baseURL)

    // Read the initial active version from the API rather than the
    // tag — the API is the source of truth and avoids racing with the
    // initial /version fetch the tag is awaiting.
    const initialVerRes = await page.request.get(
      `${baseURL}/api/hub/version`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
      },
    )
    expect(initialVerRes.ok()).toBeTruthy()
    const initialVer = (await initialVerRes.json()).hub_version as string
    expect(
      initialVer,
      'sanity: TARGET_VERSION must differ from the initial seed version, ' +
        'otherwise the activate is a no-op and the test proves nothing',
    ).not.toBe(TARGET_VERSION)

    // Device B — second context for the SAME admin user. Load fully
    // BEFORE A activates so its HubCatalog store is mounted and
    // subscribed to sync:hub_settings.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoHub(pageB, baseURL)

      // Sanity: device B's tag currently shows the initial version.
      await expect(versionTag(pageB)).toHaveText(`v${initialVer}`, {
        timeout: 15_000,
      })

      // Device A pins the target version via REST. The handler fetches
      // + cosign-verifies + rotates against the real GitHub release
      // for `v{TARGET_VERSION}`, then publishes HubSettings/Update.
      const activateRes = await page.request.post(
        `${baseURL}/api/hub/activate`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { version: TARGET_VERSION },
          // GitHub API call + tarball download + sigstore verification.
          // Cosign + decompression can be slow on a cold cache.
          timeout: 60_000,
        },
      )
      const activateBody = await activateRes.text()
      expect(
        activateRes.ok(),
        `activate v${TARGET_VERSION} failed: ${activateRes.status()} ${activateBody}`,
      ).toBeTruthy()

      // Sanity: confirm the backend really swapped versions BEFORE we
      // wait on the UI. If this fails, the bug is in the activate path,
      // not in the sync delivery.
      const afterVerRes = await page.request.get(
        `${baseURL}/api/hub/version`,
        { headers: { Authorization: `Bearer ${adminToken}` } },
      )
      const afterVer = (await afterVerRes.json()).hub_version as string
      expect(
        afterVer,
        `backend did not switch to ${TARGET_VERSION} (still ${afterVer}) — activate succeeded but had no effect`,
      ).toBe(TARGET_VERSION)

      // Device B's VersionPicker tag must reflect the new active
      // version WITHOUT a manual reload. Only true if:
      //   1. HubSettings/Update was published to hub::catalog::read
      //   2. SSE delivered the frame to B
      //   3. `reloadAllTabs()` ran → `loadVersion()` refreshed hubVersion
      //   4. VersionPicker re-rendered with the new tag text
      await expect(versionTag(pageB)).toHaveText(`v${TARGET_VERSION}`, {
        timeout: 45_000,
      })
    } finally {
      await ctxB.close()
    }
  })
})
