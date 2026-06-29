/**
 * Plan spec #2 — list-page Switch ON against a healthy mock must
 * show a success toast, the row stays enabled, and no Alert renders.
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

test('list Switch ON against a healthy mock enables the row + no Alert', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(200)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const name = uniqueRepoName()
    await seedRepository(baseURL, token, name, mock.url())

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const row = repoRow(page, name)
    const toggle = row.locator('[data-testid^="llmrepo-toggle-"]').first()
    await expect(toggle).toHaveAttribute('aria-checked', 'false')

    await toggle.click()

    await expect(page.locator('[data-sonner-toast][data-type="success"]').first()).toBeVisible({
      timeout: 10_000,
    })
    await expect(toggle).toHaveAttribute('aria-checked', 'true', {
      timeout: 10_000,
    })
    await expect(row.locator('[data-testid^="llmrepo-health-alert-"]')).toHaveCount(0)
  } finally {
    await mock.dispose()
  }
})
