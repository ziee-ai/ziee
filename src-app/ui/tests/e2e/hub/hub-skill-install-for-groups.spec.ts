import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — Skill hub card "Install for groups…" modal (admin distribution path).
 *
 * `SkillHubCard` gives `canManageSystem` admins a split Dropdown button whose
 * "Install for groups…" item opens a Dialog ("Install for groups") with a
 * multi-select of user groups; submit calls `HubSkills.installForGroups`. None
 * of the modal flow was exercised. This drives it end-to-end against the seeded
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

    const card = page.getByTestId(/^hub-skill-card-/).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    const name = (await card.getAttribute('data-testid'))!.slice(
      'hub-skill-card-'.length,
    )

    // Click the admin "Groups…" install button (separate from Install-for-me
    // and Install-as-system).
    await page.getByTestId(`hub-skill-install-groups-btn-${name}`).click()

    // The dialog opens with the group multi-select.
    const dialog = page.getByTestId(`hub-skill-groups-dialog-${name}`)
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Select the first available group.
    await page.getByTestId(`hub-skill-groups-multiselect-${name}`).click()
    await page
      .locator(`[data-testid^="hub-skill-groups-multiselect-${name}-opt-"]`)
      .first()
      .click()
    // Close the option overlay.
    await page.keyboard.press('Escape')

    // Confirm install.
    await page.getByTestId(`hub-skill-groups-install-btn-${name}`).click()

    // Success toast for the group-scoped install (`... for selected groups`).
    await expect(
      page
        .locator('[data-sonner-toast][data-type="success"]')
        .filter({ hasText: 'for selected groups' })
        .first(),
    ).toBeVisible({ timeout: 15000 })
  })
})
