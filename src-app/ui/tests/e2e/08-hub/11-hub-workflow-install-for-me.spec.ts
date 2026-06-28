import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — install a WORKFLOW from the hub (HubWorkflows.installForMe). The hub
 * Workflows tab (WorkflowHubCard, 10 seeded workflow entries) had ZERO E2E
 * coverage. This clicks the card's "Install" primary action and asserts the
 * success toast + the "Installed" badge — mirrors 10-hub-skill-install-for-me.
 */

test.describe('Hub Workflows — install for me', () => {
  test('clicking Install on a hub workflow installs it and shows the Installed badge', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/hub/workflows`)
    await expect(page).toHaveURL(/\/hub\/workflows/)
    await waitForHubDataLoad(page)

    const card = page.locator('[data-testid^="hub-workflow-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30000 })

    // Admin Dropdown.Button primary action is "Install" (for me).
    await card.getByRole('button', { name: 'Install', exact: true }).click()

    await expect(page.getByText(/^Installed "/).first()).toBeVisible({ timeout: 15000 })
    await expect(card.getByText('Installed', { exact: true })).toBeVisible({ timeout: 15000 })
  })
})
