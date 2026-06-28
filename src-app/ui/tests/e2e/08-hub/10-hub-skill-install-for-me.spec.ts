import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — install a skill FROM THE HUB through the UI (Skill.store install-from-hub
 * path). The existing hub specs cover assistants (create) and the skill
 * install-for-GROUPS modal; the basic "Install (for me)" action from a hub skill
 * card was untested. This clicks it and asserts the success toast + the card's
 * "Installed" badge.
 */

test.describe('Hub Skills — install for me', () => {
  test('clicking Install on a hub skill installs it and shows the Installed badge', async ({
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

    // The admin Dropdown.Button's primary action is "Install (for me)".
    await card.getByRole('button', { name: 'Install', exact: true }).click()

    // Success toast for the for-me install.
    await expect(page.getByText(/^Installed "/).first()).toBeVisible({
      timeout: 15000,
    })

    // The card now carries the green "Installed" badge (state → user install).
    await expect(card.getByText('Installed', { exact: true })).toBeVisible({
      timeout: 15000,
    })
  })
})
