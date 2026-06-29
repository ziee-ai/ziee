/**
 * Plan spec #3 — drawer Enabled toggle behavior.
 *
 *   First try with a 401 mock: drawer renders the body-top Alert
 *   with the failure reason, the Switch snaps back to off, the
 *   error toast surfaces the reason.
 *
 *   Flip mock to 200: clicking the Switch again enables the row;
 *   the Alert disappears.
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
} from './helpers/repository-helpers'
import { RepoHealthMock } from './helpers/repository-health-mock'
import { byTestId } from '../testid'
import {
  seedRepository,
  uniqueRepoName,
  repoRow,
} from './helpers/repository-health-helpers'

test('drawer Enabled toggle: 401 reverts + Alert; flipping mock to 200 enables', async ({
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
    await row.locator('[data-testid^="llmrepo-edit-btn-"]').first().click()

    await byTestId(page, 'llmrepo-form').waitFor({ state: 'visible' })

    const drawerSwitch = byTestId(page, 'llmrepo-form-enabled-switch')
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'false')

    await drawerSwitch.click()

    // 401 → error toast + Switch snaps back off + body-top Alert renders.
    await expect(page.locator('[data-sonner-toast][data-type="error"]').first()).toBeVisible({
      timeout: 15_000,
    })
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'false', {
      timeout: 10_000,
    })
    await expect(byTestId(page, 'llmrepo-drawer-health-alert').first()).toBeVisible({
      timeout: 10_000,
    })

    // Flip mock to 200; click again — Switch sticks + Alert disappears.
    mock.respondWith(200)
    await drawerSwitch.click()
    await expect(page.locator('[data-sonner-toast][data-type="success"]').first()).toBeVisible({
      timeout: 10_000,
    })
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'true', {
      timeout: 10_000,
    })
    await expect(byTestId(page, 'llmrepo-drawer-health-alert')).toHaveCount(0)
  } finally {
    await mock.dispose()
  }
})
