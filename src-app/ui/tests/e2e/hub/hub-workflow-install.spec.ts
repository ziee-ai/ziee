import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * Hub install flow for WORKFLOWS via the UI (WorkflowHubCard). The backend
 * install is covered by tests/workflow/install_from_hub.rs and the Import
 * (dev-bundle) button by workflows; the hub→install UI path itself had no
 * E2E. Uses the embedded hub seed (no network).
 */
test.describe('Hub - workflow install', () => {
  test('installing a workflow from the hub shows success + Installed tag', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/hub/workflows`)
    await expect(page).toHaveURL(/\/hub\/workflows/)
    await waitForHubDataLoad(page)

    // The embedded seed ships several workflows; grab the first card + its
    // install button (admin split-button primary or plain install).
    const card = page.getByTestId(/^hub-workflow-card-/).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    const installBtn = card.locator(
      '[data-testid^="hub-workflow-install-dropdown-btn-"], [data-testid^="hub-workflow-install-btn-"]',
    )
    await expect(installBtn).toBeVisible()
    await installBtn.click()
    await page.keyboard.press('Escape')

    // Success toast (`Installed "<title>"`), then the card flips to "Installed".
    await expect(
      page
        .locator('[data-sonner-toast][data-type="success"]')
        .filter({ hasText: 'Installed' })
        .first(),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      card.locator('[data-testid^="hub-workflow-installed-tag-"]'),
    ).toBeVisible({ timeout: 10000 })
  })
})
