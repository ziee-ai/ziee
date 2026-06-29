import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUserGroups,
  openCreateGroupDrawer,
} from './helpers/user-navigation'
import { createGroup } from './helpers/group-actions'

/**
 * E2E — GroupListItem status badge + System tag (audit abe1955390c5f570).
 * GroupListItem renders a "System" Tag for built-in groups and an active /
 * inactive status badge + label.
 */

test.describe('User Groups — status badges', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await navigateToUserGroups(page, testInfra.baseURL)
  })

  test('system groups show the System tag and an active badge', async ({
    page,
  }) => {
    // The seeded built-in groups (Administrators / Users) are is_system.
    const card = page
      .locator('[data-testid^="user-group-card-"]')
      .filter({
        has: page.locator('[data-testid^="user-group-system-tag-"]'),
      })
      .first()
    await expect(card).toBeVisible({ timeout: 15000 })
    await expect(
      card.locator('[data-testid^="user-group-system-tag-"]'),
    ).toBeVisible()
    await expect(
      card.locator('[data-testid^="user-group-active-badge-"]'),
    ).toBeVisible()
    await expect(
      card.locator('[data-testid^="user-group-status-text-"]'),
    ).toHaveText(/^active$/i)
  })

  test('a custom group shows an active badge and no System tag', async ({
    page,
  }) => {
    const groupName = `BadgeGrp${Date.now()}`
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'badge test' })

    const row = byTestId(page, `user-group-row-${groupName}`)
    await expect(row).toBeVisible({ timeout: 10000 })

    // Active → active badge + "Active" label.
    await expect(
      row.locator('[data-testid^="user-group-active-badge-"]'),
    ).toBeVisible()
    await expect(
      row.locator('[data-testid^="user-group-status-text-"]'),
    ).toHaveText(/^active$/i)
    // A user-created group is not a system group → no System tag.
    await expect(
      row.locator('[data-testid^="user-group-system-tag-"]'),
    ).toHaveCount(0)
  })
})
