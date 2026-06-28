import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the SandboxFlavors selector inside McpUserPolicyCard (the user-stdio
 * sandbox flavor, McpUserPolicyCard.tsx:55,88-98). The card + its flavor Select
 * (fed by the SandboxFlavors store) had ZERO E2E coverage. Allowing the stdio
 * transport reveals the flavor Select; picking a flavor + saving must persist
 * the policy. (multiUserMode defaults true in the web build, so the card
 * renders; the flavor options fall back to the static KNOWN_FLAVORS catalog.)
 */

test.describe('MCP user policy — stdio sandbox flavor selector', () => {
  test('allowing stdio reveals the flavor selector and a chosen flavor saves', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/mcp-admin`)

    const card = page.getByTestId('mcp-user-policy-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    // Allow the stdio transport → the sandbox-flavor Select appears.
    const stdio = card.getByRole('checkbox', { name: 'Standard I/O (sandboxed)' })
    if (!(await stdio.isChecked())) await stdio.check()

    await expect(card.getByText('User stdio sandbox flavor')).toBeVisible({ timeout: 10000 })

    // Pick the "minimal" flavor from the SandboxFlavors-backed Select.
    await card.locator('.ant-select').click()
    await page.getByRole('option', { name: 'minimal', exact: true }).click()

    // Save → policy persists.
    await card.getByRole('button', { name: 'Save policy' }).click()
    await expect(page.getByText('MCP user policy updated')).toBeVisible({ timeout: 10000 })
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
