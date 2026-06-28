import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * Hub install flow for WORKFLOWS via the UI (WorkflowHubCard). The backend
 * install is covered by tests/workflow/install_from_hub.rs and the Import
 * (dev-bundle) button by 17-workflows; the hub→install UI path itself had no
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
    // install button.
    const card = page.getByTestId(/^hub-workflow-card-/).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    const installBtn = card.getByTestId('hub-workflow-install-btn')
    await expect(installBtn).toBeVisible()
    await installBtn.click()

    // Success toast, then the card flips to the "Installed" state.
    await expect(page.getByText(/^Installed "/)).toBeVisible({ timeout: 10000 })
    await expect(card.getByText('Installed', { exact: true })).toBeVisible({
      timeout: 10000,
    })
  })
})
