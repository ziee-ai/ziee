/**
 * Plan spec #7 — EDIT drawer's Test Connection button persists the
 * outcome to `last_health_check_*` columns and the drawer's inline
 * `<Alert>` reflects the result without a page reload.
 *
 *   - Seed a healthy enabled row pointing at a 200 mock
 *   - Open Edit drawer; assert no Alert visible
 *   - Flip mock to 401
 *   - Click Test Connection button
 *   - Assert: error toast, the row's persisted
 *     last_health_check_status flips to 'unhealthy' (verify via API),
 *     drawer's Alert renders without reload
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

async function readRepoStatus(
  apiURL: string,
  token: string,
  repoId: string,
): Promise<string> {
  const resp = await fetch(`${apiURL}/api/llm-repositories/${repoId}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!resp.ok) throw new Error(`get failed: ${resp.status}`)
  const body = await resp.json()
  return body.last_health_check_status as string
}

test('edit-mode Test Connection button persists outcome + renders Alert', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(200)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const name = uniqueRepoName()
    const repoId = await seedRepository(baseURL, token, name, mock.url())

    // Status starts at 'untested' (the create-flow probe doesn't run
    // when enabled=false).
    expect(await readRepoStatus(baseURL, token, repoId)).toBe('untested')

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const row = repoRow(page, name)
    await row.locator('[data-testid^="llmrepo-edit-btn-"]').first().click()

    await byTestId(page, 'llmrepo-form').waitFor({ state: 'visible' })

    // No Alert on a fresh edit drawer for an 'untested' row.
    await expect(byTestId(page, 'llmrepo-drawer-health-alert')).toHaveCount(0)

    // Flip mock to 401 + click Test Connection — the button only
    // appears when URL + auth are populated; the row has both.
    mock.respondWith(401)
    const testButton = byTestId(page, 'llmrepo-form-test-btn')
    await expect(testButton).toBeVisible({ timeout: 10_000 })
    await testButton.click()

    // Error toast surfaces the failure.
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]').first(),
    ).toBeVisible({ timeout: 15_000 })

    // Persisted status flipped to unhealthy (verify via API — the UI
    // refetches via the `updated` event, but DB is the source of
    // truth).
    await expect.poll(
      async () => readRepoStatus(baseURL, token, repoId),
      { timeout: 10_000 },
    ).toBe('unhealthy')

    // Drawer's inline Alert renders without manual reload.
    await expect(
      byTestId(page, 'llmrepo-drawer-health-alert').first(),
    ).toBeVisible({ timeout: 10_000 })
  } finally {
    await mock.dispose()
  }
})
