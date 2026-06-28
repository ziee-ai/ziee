import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * E2E — Hub Workflows tab install flow (audit id 296e4da0a5aecb18).
 * The hub had E2E specs for assistants / mcp / models / skills but none for
 * WORKFLOWS; the install-from-hub path (WorkflowHubCard → installFromHub →
 * create-from-hub) was untested through the UI. The seed catalog ships 10
 * workflows.
 */

test.describe('Hub Workflows', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/hub/workflows`)
    await page.waitForLoadState('load')
    await waitForHubDataLoad(page)
  })

  test('lists hub workflows and installs one for the current user', async ({ page }) => {
    const card = page.locator('[data-testid^="hub-workflow-card-"]').first()
    await expect(card).toBeVisible({ timeout: 20000 })

    // Not yet installed.
    await expect(card.getByText('Installed', { exact: true })).toHaveCount(0)

    // The card's install action (Dropdown.Button primary / "Install for me").
    await card.getByTestId('hub-workflow-install-btn').click()

    // Real install → success toast.
    await expect(page.locator('.ant-message-success')).toBeVisible({ timeout: 10000 })

    // Card reflects the installed state.
    await expect(card.getByText('Installed', { exact: true })).toBeVisible({
      timeout: 10000,
    })
  })
})
