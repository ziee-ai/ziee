import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  getAdminToken,
  createTestUser,
  clearAuthState,
} from '../../common/auth-helpers'
import { goToProjectsPage, getProjectCard } from './helpers/project-helpers'

/**
 * E2E — per-permission gating of the ProjectCard action buttons for a
 * NON-admin user (audit all-6c0c71a334e0).
 *
 * Every other 11-projects spec logs in as admin (wildcard perms), so the
 * `usePermission`-gated Edit / Duplicate / Delete icon buttons in
 * `ProjectCard.tsx` were only ever exercised in their VISIBLE state. This
 * spec drives the hidden state: a user holding only `projects::read` can
 * SEE the project (the list + card render) but must NOT see any of the
 * three mutating actions — Edit (projects::edit), Delete (projects::delete),
 * or Duplicate (projects::create && projects::read). A positive control
 * confirms admin still sees them, so a regression that dropped the gate
 * (showing the buttons to everyone) fails here.
 */

async function seedProject(
  apiURL: string,
  token: string,
  name: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`seed project failed: ${res.status}`)
  return (await res.json()).id
}

test.describe('Projects — non-admin permission gating on ProjectCard', () => {
  test('read-only user sees the project but none of the gated action buttons', async ({
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
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const projectName = `ReadOnly Target ${tag}`
    await seedProject(apiURL, adminToken, projectName)

    // A non-admin who can READ projects but cannot create/edit/delete.
    const uname = `proj_ro_${tag}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@ex.com`, 'password123', [
      'profile::read',
      'profile::edit',
      'projects::read',
    ])

    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await goToProjectsPage(page, baseURL)

    // READ works: the seeded project's card renders for this user.
    const card = getProjectCard(page, projectName)
    await expect(card).toBeVisible({ timeout: 15000 })

    // But every mutating action button is gated away — none render.
    await expect(
      card.getByRole('button', { name: `Edit ${projectName}` }),
    ).toHaveCount(0)
    await expect(
      card.getByRole('button', { name: `Duplicate ${projectName}` }),
    ).toHaveCount(0)
    await expect(
      card.getByRole('button', { name: `Delete ${projectName}` }),
    ).toHaveCount(0)
  })

  test('positive control: admin sees the Edit + Delete + Duplicate actions', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const projectName = `Admin Visible ${tag}`
    await seedProject(apiURL, adminToken, projectName)

    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    const card = getProjectCard(page, projectName)
    await expect(card).toBeVisible({ timeout: 15000 })

    // Admin holds the `*` wildcard → all three gated actions render.
    await expect(
      card.getByRole('button', { name: `Edit ${projectName}` }),
    ).toBeVisible()
    await expect(
      card.getByRole('button', { name: `Duplicate ${projectName}` }),
    ).toBeVisible()
    await expect(
      card.getByRole('button', { name: `Delete ${projectName}` }),
    ).toBeVisible()
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
