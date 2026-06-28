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
  })
})
