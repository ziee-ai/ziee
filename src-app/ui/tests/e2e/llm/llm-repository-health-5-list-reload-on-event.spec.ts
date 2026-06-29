/**
 * Plan spec #5 — the `llm_repository.auto_disabled` event triggers a
 * list reload so the row's `last_health_check_status` flips from
 * 'untested' to 'unhealthy' in the visible DOM without a manual
 * refresh.
 *
 * Drives the failure path (Switch click → 401 mock → backend reverts
 * + emits event → store re-fetches) and asserts the Alert appears
 * without `page.reload()`.
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

test('list reloads on auto_disabled event without a manual refresh', async ({
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
    // Initial mount: status is 'untested' (no Alert).
    await expect(row.locator('[data-testid^="llmrepo-health-alert-"]')).toHaveCount(0)

    // Click Switch → backend probe fails → auto_disabled event →
    // store re-fetches list → row now has 'unhealthy' status →
    // Alert renders WITHOUT page.reload().
    await row.locator('[data-testid^="llmrepo-toggle-"]').first().click()

    await expect(row.locator('[data-testid^="llmrepo-health-alert-"]').first()).toBeVisible({
      timeout: 10_000,
    })
  } finally {
    await mock.dispose()
  }
})
