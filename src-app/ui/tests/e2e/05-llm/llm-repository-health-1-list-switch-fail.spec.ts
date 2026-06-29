/**
 * Plan spec #1 — list-page Switch ON against a failing mock must
 * surface an error toast, leave the row disabled, and render the
 * inline Alert. Drives the live backend → in-process Node mock that
 * returns 401, then walks through the UI.
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
} from './helpers/repository-helpers'
import { RepoHealthMock } from './helpers/repository-health-mock'
import {
  seedRepository,
  uniqueRepoName,
  repoRow,
} from './helpers/repository-health-helpers'

test('list Switch ON against a failing mock shows error toast + Alert and stays disabled', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(401)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const name = uniqueRepoName()
    await seedRepository(baseURL, token, name, mock.url())

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const row = repoRow(page, name)
    await expect(row).toBeVisible()
    const toggle = row.locator('[data-testid^="llmrepo-toggle-"]').first()
    await expect(toggle).toHaveAttribute('aria-checked', 'false')

    await toggle.click()

    // Error toast surfaces the probe reason.
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]').first(),
    ).toBeVisible({ timeout: 15_000 })

    // The store's auto_disabled listener re-fetches; the row's
    // last_health_check_status is now 'unhealthy' and the Alert
    // renders inline.
    await expect(toggle).toHaveAttribute('aria-checked', 'false')
    await expect(row.locator('[data-testid^="llmrepo-health-alert-"]').first()).toBeVisible({
      timeout: 10_000,
    })
  } finally {
    await mock.dispose()
  }
})
