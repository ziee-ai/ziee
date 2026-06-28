import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  getCurrentUserToken,
} from '../../common/auth-helpers'
import { goToProjectsPage } from './helpers/project-helpers'

/**
 * Non-admin project permissions — every other 11-projects spec logs in as
 * admin (all perms via *). ProjectCard gates Edit/Duplicate/Delete on
 * projects::edit / (create+read) / delete respectively. A user holding
 * read+create but NOT edit/delete must see the card + Duplicate, but neither
 * Edit nor Delete.
 */
test.describe('Projects - non-admin permission gating', () => {
  test('read+create user sees Duplicate but not Edit/Delete on a project card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const username = `proj_ro_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'projects::read', 'projects::create'],
    )
    await login(page, baseURL, username, 'password123')

    // The user creates their own project (they hold projects::create).
    const userToken = await getCurrentUserToken(page)
    const created = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${userToken}` },
      data: { name: 'Gated Project' },
    })
    expect(created.ok()).toBe(true)

    await goToProjectsPage(page, baseURL)
    const card = page.locator('[data-test-project-name="Gated Project"]')
    await expect(card).toBeVisible({ timeout: 15000 })

    // Duplicate is gated on create+read → visible.
    await expect(
      card.getByRole('button', { name: 'Duplicate Gated Project' }),
    ).toBeVisible()
    // Edit (projects::edit) and Delete (projects::delete) are NOT held → hidden.
    await expect(
      card.getByRole('button', { name: 'Edit Gated Project' }),
    ).toHaveCount(0)
    await expect(
      card.getByRole('button', { name: 'Delete Gated Project' }),
    ).toHaveCount(0)
  })
})
