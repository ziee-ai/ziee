import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — Skill hub card "Install for groups…" modal (admin distribution path).
 *
 * `SkillHubCard` gives `canManageSystem` admins a `Dropdown.Button` whose
 * "Install for groups…" item opens a Modal ("Install for groups") with a
 * multi-select of user groups; OK calls `HubSkills.installForGroups`. None of
 * the modal flow was exercised. This drives it end-to-end against the seeded
 * hub skill catalog.
 */

test.describe('Hub Skills — install for groups', () => {
  test('admin opens the "Install for groups" modal and installs for a group', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/hub/skills`)
    await expect(page).toHaveURL(/\/hub\/skills/)
    await waitForHubDataLoad(page)

    const card = page.locator('[data-testid^="hub-skill-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30000 })

    // Open the admin install dropdown (the arrow part of the Dropdown.Button).
    await card.locator('.ant-dropdown-trigger').click()

    // Pick "Install for groups…" from the menu.
    await page
      .getByRole('menuitem', { name: /Install for groups/i })
      .click()

    // The modal opens with the group multi-select.
    const modal = page.getByRole('dialog', { name: 'Install for groups' })
    await expect(modal).toBeVisible({ timeout: 10000 })

    // Select the first available group.
    await modal.locator('.ant-select').click()
    await page
      .locator('.ant-select-dropdown:visible .ant-select-item-option')
      .first()
      .click()
    // Close the option overlay.
    await page.keyboard.press('Escape')

    // Confirm install.
    await modal.getByRole('button', { name: 'Install', exact: true }).click()

    // Success toast for the group-scoped install.
    await expect(
      page.getByText(/for selected groups/i).first(),
    ).toBeVisible({ timeout: 15000 })
  })
})
