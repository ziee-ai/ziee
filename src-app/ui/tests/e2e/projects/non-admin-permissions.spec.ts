import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  clearAuthState,
} from '../../common/auth-helpers'
import { goToProjectsPage, getProjectCard } from './helpers/project-helpers'

/**
 * E2E — per-permission gating of the ProjectCard action buttons for a
 * NON-admin user (audit all-6c0c71a334e0).
 *
 * Every other projects spec logs in as admin (wildcard perms), so the
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
  test('user without edit/delete perms sees the project but not the Edit/Delete buttons', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const projectName = `ReadOnly Target ${tag}`

    // A non-admin who can READ + CREATE projects (so they can own one — projects
    // are owner-scoped) but CANNOT edit or delete. Duplicate follows create.
    const uname = `proj_ro_${tag}`
    await createTestUser(apiURL, adminToken, uname, `${uname}@ex.com`, 'password123', [
      'profile::read',
      'profile::edit',
      'projects::read',
      'projects::create',
    ])

    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')

    // Projects are owner-scoped (list_for_user), so the project must be owned by
    // THIS user to appear in their list — seed it with the user's own token.
    const userToken = await getCurrentUserToken(page)
    await seedProject(apiURL, userToken, projectName)
    await goToProjectsPage(page, baseURL)

    // READ works: the seeded project's card renders for this user.
    const card = getProjectCard(page, projectName)
    await expect(card).toBeVisible({ timeout: 15000 })

    // Edit + Delete are gated away (no projects::edit / projects::delete)...
    await expect(
      card.locator('[data-testid^="project-card-edit-button-"]'),
    ).toHaveCount(0)
    // ...while Duplicate is gated by projects::create, which this user HAS.
    await expect(
      card.locator('[data-testid^="project-card-duplicate-button-"]'),
    ).toHaveCount(1)
    await expect(
      card.locator('[data-testid^="project-card-delete-button-"]'),
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
      card.locator('[data-testid^="project-card-edit-button-"]'),
    ).toBeVisible()
    await expect(
      card.locator('[data-testid^="project-card-duplicate-button-"]'),
    ).toBeVisible()
    await expect(
      card.locator('[data-testid^="project-card-delete-button-"]'),
    ).toBeVisible()
  })
})
