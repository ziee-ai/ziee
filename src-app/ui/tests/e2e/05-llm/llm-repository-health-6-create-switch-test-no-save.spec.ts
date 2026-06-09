/**
 * Plan spec #6 — Add Repository drawer's Enable switch probes the
 * form values WITHOUT persisting a row.
 *
 *   - Open Add Repository drawer
 *   - Fill required fields with URL pointing at a failing mock
 *   - Flip Enable switch ON
 *   - Assert: error toast, switch stays OFF, NO row was created
 *     (verify via API list count before + after)
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
  openAddRepositoryDrawer,
} from './helpers/repository-helpers'
import { RepoHealthMock } from './helpers/repository-health-mock'

async function listRepositoryCount(
  apiURL: string,
  token: string,
): Promise<number> {
  const resp = await fetch(`${apiURL}/api/llm-repositories?page=1&per_page=100`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!resp.ok) throw new Error(`list failed: ${resp.status}`)
  const body = await resp.json()
  return body.total as number
}

test('create-mode Enable switch tests the form WITHOUT persisting on failure', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(401)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const before = await listRepositoryCount(baseURL, token)

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)
    await openAddRepositoryDrawer(page)

    // Fill the form with the failing mock's URL. Auth type stays
    // 'none' (drawer default) — no secret fields needed.
    const name = `create-test-${Math.random().toString(36).slice(2, 8)}`
    await page.fill('#llm-repository-form_name', name)
    await page.fill('#llm-repository-form_url', mock.url())

    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    // The "Enable Repository" label maps to the visible Switch under
    // the hidden form field — locate via the new aria-label.
    const drawerSwitch = drawer.locator(
      'button.ant-switch[aria-label="Enable repository"]',
    )
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'true')

    // Switch is ON by default for create mode. Toggle OFF first so
    // the next ON click triggers the probe-on-toggle flow.
    await drawerSwitch.click()
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'false')

    // Now toggle ON — probe should fire against the 401 mock.
    await drawerSwitch.click()

    // Error toast + switch snaps back to OFF.
    await expect(
      page.locator('.ant-message-error').first(),
    ).toBeVisible({ timeout: 15_000 })
    await expect(drawerSwitch).toHaveAttribute('aria-checked', 'false', {
      timeout: 10_000,
    })

    // Verify NO row was created — the probe didn't persist anything.
    const after = await listRepositoryCount(baseURL, token)
    expect(after).toBe(before)
  } finally {
    await mock.dispose()
  }
})
