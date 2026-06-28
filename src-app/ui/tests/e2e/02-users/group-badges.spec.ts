import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUserGroups,
  openCreateGroupDrawer,
} from './helpers/user-navigation'
import { createGroup } from './helpers/group-actions'

/**
 * E2E — GroupListItem status badge + System tag colors
 * (audit id abe1955390c5f570). GroupListItem renders an orange "System" Tag for
 * built-in groups and a Badge (status=success "Active" / default "Inactive").
 * The user-status spec covers USER badge colors, but the GROUP badge/tag colors
 * were never asserted.
 */

test.describe('User Groups — status badges', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await navigateToUserGroups(page, testInfra.baseURL)
  })

  test('system groups show the orange "System" tag and an active badge', async ({ page }) => {
    // The seeded built-in groups (Administrators / Users) are is_system → an
    // orange Tag, and is_active → a success-status Badge.
    await expect(
      page.locator('.ant-tag-orange', { hasText: 'System' }).first(),
    ).toBeVisible({ timeout: 15000 })
    await expect(
      page.locator('.ant-badge-status-success').first(),
    ).toBeVisible()
  })

  test('a custom group shows an active (success) badge and no System tag', async ({ page }) => {
    const groupName = `BadgeGrp${Date.now()}`
    await openCreateGroupDrawer(page)
    await createGroup(page, { name: groupName, description: 'badge test' })

    // Scope to the new group's row via its unique Edit button ancestor.
    const editButton = page.getByRole('button', {
      name: `Edit ${groupName}`,
      exact: true,
    })
    await expect(editButton).toBeVisible({ timeout: 10000 })
    const groupRow = editButton.locator(
      'xpath=ancestor::div[contains(@class, "ant-card-body")][1]',
    )

    // Active → success-status badge.
    await expect(groupRow.locator('.ant-badge-status-success')).toBeVisible()
    await expect(groupRow.getByText('Active')).toBeVisible()
    // A user-created group is not a system group → no orange System tag.
    await expect(groupRow.locator('.ant-tag-orange')).toHaveCount(0)
  })
})
