import { test, expect } from './no-403'
import { loginAsMember, loginAsUsersReadOnly } from './fixtures'

test.describe('users module — permission gating', () => {
  test('non-admin: Users settings entry is hidden + deep-link renders 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // Sidebar Settings exists for everyone (it has user-scope pages too),
    // but the Users / User Groups admin entries should NOT be in the
    // settings menu for a non-admin.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      page.getByRole('menuitem', { name: /^Users$/ }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('menuitem', { name: /^User Groups$/ }),
    ).toHaveCount(0)

    // Deep-link directly to the page — should render the inline 403,
    // URL preserved. Ant Design's <Result status="403"> renders "403"
    // inside an SVG, not as a text node, so assert against the title /
    // subtitle text the router-level RoutePermissionGate emits.
    await page.goto(`${testInfra.baseURL}/settings/users`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible()
    expect(page.url()).toContain('/settings/users')
  })

  test('read-only: list visible, Create + Edit + Delete absent', async ({
    page,
    testInfra,
  }) => {
    await loginAsUsersReadOnly(page, testInfra.baseURL, testInfra.apiURL)

    await page.goto(`${testInfra.baseURL}/settings/users`)
    await expect(page.getByText(/users/i).first()).toBeVisible()

    // Create user '+' button — gated on users::create
    await expect(
      page.getByRole('button', { name: /create user/i }),
    ).toHaveCount(0)

    // Per-row actions — none of these should appear without ::edit /
    // ::delete / ::reset_password
    await expect(
      page.getByRole('button', { name: /^edit$/i }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('button', { name: /reset password/i }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('button', { name: /^delete/i }),
    ).toHaveCount(0)
  })
})
