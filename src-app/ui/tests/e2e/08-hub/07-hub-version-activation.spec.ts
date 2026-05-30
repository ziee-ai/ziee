import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, switchHubTab, waitForHubDataLoad } from './helpers/hub-navigation'

// Admin catalog-version pinning. Exercises the VersionPicker dropdown in
// the HubPage header, the Updates tab, and the Incompatible(N) footer.
//
// These flows hit the real ziee-ai/hub GitHub Releases (activate does a
// download + cosign verify), same network dependency as the backend
// refresh integration test. Two published versions are assumed to exist:
//   v0.0.1-alpha (13 items) and v0.0.2-alpha (16 items, +linear-mcp which
//   pins min_ziee_version 99.0.0 so it lands in the Incompatible footer).

test.describe('Hub version activation (admin)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
  })

  test('admin sees the version picker in the header', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // Use the stable testid (verb-only aria-label also present).
    const picker = page.getByTestId('hub-version-picker')
    await expect(picker).toBeVisible({ timeout: 15000 })
    // Shows the currently-installed version tag (seed = v0.0.1-alpha).
    await expect(picker).toContainText(/v0\.0\.\d/)
  })

  test('admin can list and activate a catalog version', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // Open the picker — lazy-loads the version list from GitHub.
    await page.getByRole('button', { name: /select hub catalog version/i }).click()

    // The menu should list both published versions + "Track latest".
    await expect(page.getByText('Track latest')).toBeVisible()
    const v2 = page.getByRole('menuitem', { name: /v0\.0\.2-alpha/ })
    await expect(v2).toBeVisible({ timeout: 15000 })

    // Activate v0.0.2-alpha — triggers fetch + cosign verify + rotate.
    await v2.click()

    // Success toast, then the header tag reflects the new version.
    await expect(page.getByText(/activated hub catalog v0\.0\.2-alpha/i)).toBeVisible({
      timeout: 30000,
    })
    await expect(
      page.getByRole('button', { name: /select hub catalog version/i }),
    ).toContainText('v0.0.2-alpha')
  })

  test('incompatible items are hidden after activating v0.0.2', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    await page.getByRole('button', { name: /select hub catalog version/i }).click()
    const v2 = page.getByRole('menuitem', { name: /v0\.0\.2-alpha/ })
    await expect(v2).toBeVisible({ timeout: 15000 })
    await v2.click()
    await expect(page.getByText(/activated hub catalog v0\.0\.2-alpha/i)).toBeVisible({
      timeout: 30000,
    })

    // v0.0.2-alpha ships items pinned to min_ziee_version 99.0.0
    // (deepseek-r1-70b, deep-researcher, linear-mcp). They must NOT
    // appear in any tab — incompatible items are hidden entirely.
    await waitForHubDataLoad(page)
    await expect(page.getByText('deepseek-r1-70b')).toHaveCount(0)

    await switchHubTab(page, 'assistants')
    await waitForHubDataLoad(page)
    await expect(page.getByText('Deep Researcher')).toHaveCount(0)

    await switchHubTab(page, 'mcp-servers')
    await waitForHubDataLoad(page)
    await expect(page.getByText('Linear MCP Server')).toHaveCount(0)
  })

  test('admin sees the Updates tab', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)

    // The Updates tab is admin-only (gated on hub::catalog::read). It appears in
    // the segmented tab control.
    await page.goto(`${baseURL}/hub/updates`)
    await expect(page).toHaveURL(/\/hub\/updates/)
    // With nothing installed-then-outdated, the empty state shows.
    await expect(
      page.getByText(/every installed hub item is on the current catalog|behind catalog/i),
    ).toBeVisible({ timeout: 10000 })
  })
})
