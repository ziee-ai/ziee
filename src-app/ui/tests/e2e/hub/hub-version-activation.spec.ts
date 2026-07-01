import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'

// Hub catalog version surface.
//
// NOTE: the admin VersionPicker / catalog-version activation flow was REMOVED
// in Hub v2 — per-entry semver (each item carries its own min_ziee_version)
// replaced the pinnable, globally-activatable catalog version. HubPage now
// renders the installed version only as a read-only diagnostic Tag
// (`hub-version-tag`), identical for admins and users, with no dropdown / no
// activate / no track-latest. The three obsolete tests that drove that picker
// (list+activate, incompatible-after-activate, switch-back) were dropped — the
// UI and the endpoints they exercised no longer exist. The two surviving,
// still-meaningful behaviors are kept below.

test.describe('Hub catalog version surface (admin)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
  })

  test('admin sees the installed catalog version tag in the header', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // Read-only diagnostic tag showing the currently-installed catalog
    // version. Match any semver (the bundled seed version bumps over time —
    // don't pin a specific major/minor).
    const versionTag = page.getByTestId('hub-version-tag')
    await expect(versionTag).toBeVisible({ timeout: 15000 })
    await expect(versionTag).toContainText(/v\d+\.\d+\.\d+/)
  })

  test('admin sees the Installed tab with per-category empty hints', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // The Installed tab shows every tracked install visible to the caller.
    // With nothing installed in this fresh test env, the three category
    // cards render their own empty hints.
    await page.goto(`${baseURL}/hub/installed`)
    await expect(page).toHaveURL(/\/hub\/installed/)
    await expect(page.getByTestId('hub-installed-empty-model')).toBeVisible({
      timeout: 10000,
    })
  })
})
