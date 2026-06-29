import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — Hub Skills tab install flow (audit id dbd5e73bbfd3).
 * There were hub E2E specs for assistants / mcp / models but none for SKILLS;
 * the install-from-hub path (SkillHubCard → installFromHub →
 * POST /api/hub/skills create) was untested through the UI. The seed hub
 * catalog ships one skill (io.github.ziee/effective-prompting).
 */

test.describe('Hub Skills', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/hub/skills`)
    await page.waitForLoadState('load')
    await waitForHubDataLoad(page)
  })

  test('lists hub skills and installs one for the current user', async ({ page }) => {
    const card = page.getByTestId(/^hub-skill-card-/).first()
    await expect(card).toBeVisible({ timeout: 20000 })

    // Not yet installed → no "Installed" tag.
    await expect(
      card.locator('[data-testid^="hub-skill-installed-tag-"]'),
    ).toHaveCount(0)

    // Admin sees a split Dropdown button whose primary action is install-for-me
    // (handleInstallForMe → installForMe → POST create-from-hub). Non-admin
    // sees the plain "Install for me" button; match either by id prefix.
    await card
      .locator(
        '[data-testid^="hub-skill-install-dropdown-btn-"], [data-testid^="hub-skill-install-btn-"]',
      )
      .click()
    // The split button is also the menu trigger; dismiss the opened menu.
    await page.keyboard.press('Escape')

    // Success toast confirms the real install.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 10000 })

    // The card reflects the installed state with the green tag (the store
    // pushed the new skill → state becomes 'user').
    await expect(
      card.locator('[data-testid^="hub-skill-installed-tag-"]'),
    ).toBeVisible({ timeout: 10000 })
  })
})
