import { test, expect } from './no-403'
import { loginAsMember, loginAsUsersReadOnly } from './fixtures'
import { byTestId } from '../testid'

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
      byTestId(page, 'settings-nav-menu-item-users'),
    ).toHaveCount(0)
    await expect(
      byTestId(page, 'settings-nav-menu-item-user-groups'),
    ).toHaveCount(0)

    // Deep-link directly to the page — should render the inline 403,
    // URL preserved (the router gate emits router-route-forbidden-result).
    await page.goto(`${testInfra.baseURL}/settings/users`)
    await expect(byTestId(page, 'router-route-forbidden-result')).toBeVisible()
    expect(page.url()).toContain('/settings/users')
  })

  test('read-only: list visible, Create + Edit + Delete absent', async ({
    page,
    testInfra,
  }) => {
    await loginAsUsersReadOnly(page, testInfra.baseURL, testInfra.apiURL)

    await page.goto(`${testInfra.baseURL}/settings/users`)
    // The users list renders for a read-only holder.
    await expect(byTestId(page, 'user-list-card')).toBeVisible()

    // Create user button — gated on users::create
    await expect(byTestId(page, 'user-create-open-button')).toHaveCount(0)

    // Per-row actions (derived ids `user-{edit,reset-password,delete}-button-<id>`)
    // — none should appear without ::edit / ::reset_password / ::delete.
    await expect(page.getByTestId(/^user-edit-button-/)).toHaveCount(0)
    await expect(page.getByTestId(/^user-reset-password-button-/)).toHaveCount(0)
    await expect(page.getByTestId(/^user-delete-button-/)).toHaveCount(0)
  })
})
