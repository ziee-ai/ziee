import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the MCP User Policy card's stdio sandbox-flavor selector
 * (McpUserPolicyCard.tsx, on /settings/mcp-admin). The card only renders
 * in multi-user mode (the web build default).
 *
 * Covers the SandboxFlavors store (its `selectOptions` populate the Select)
 * and the stdio→flavor coupling: enabling the "Standard I/O (sandboxed)"
 * transport reveals the flavor picker, and saving without a flavor is
 * rejected client-side with a clear error.
 */

test.describe('MCP user policy — stdio sandbox flavor', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/mcp-admin`)
    await expect(
      page.getByTestId('mcp-user-policy-card'),
    ).toBeVisible({ timeout: 30000 })
  })

  test('enabling stdio reveals the flavor picker; empty flavor blocks save', async ({
    page,
  }) => {
    const card = page.getByTestId('mcp-user-policy-card')

    // Enable the sandboxed-stdio transport → the flavor Select appears.
    await card.getByRole('checkbox', { name: 'Standard I/O (sandboxed)' }).check()
    const flavorSelect = card.getByText('Pick a flavor')
    await expect(flavorSelect).toBeVisible()

    // Saving with stdio allowed but NO flavor picked is rejected client-side.
    await card.getByRole('button', { name: 'Save policy' }).click()
    await expect(
      page.getByText('Pick a sandbox flavor when stdio is allowed for users.'),
    ).toBeVisible()
  })

  test('picking a flavor from the SandboxFlavors catalog saves the policy', async ({
    page,
  }) => {
    const card = page.getByTestId('mcp-user-policy-card')
    await card.getByRole('checkbox', { name: 'Standard I/O (sandboxed)' }).check()

    // Open the flavor Select (options come from the SandboxFlavors store —
    // the admin /api/code-sandbox/flavors catalog, or the full/minimal
    // fallback) and choose the first one.
    await card.locator('.ant-select').click()
    const option = page.locator('.ant-select-item-option').first()
    await expect(option).toBeVisible()
    await option.click()

    await card.getByRole('button', { name: 'Save policy' }).click()
    await expect(page.getByText('MCP user policy updated')).toBeVisible()
  })
})
