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
    // The User-Policy card lives in the (non-default) "Policy" tab. The kit Tabs
    // lazy-render only the active panel, so the card isn't in the DOM until the
    // Policy tab is activated — activate it before waiting for the card.
    await page.getByTestId('mcp-system-tabs-tab-policy').click()
    await expect(
      page.getByTestId('mcp-user-policy-card'),
    ).toBeVisible({ timeout: 30000 })
  })

  test('enabling stdio reveals the flavor picker; empty flavor blocks save', async ({
    page,
  }) => {
    const card = page.getByTestId('mcp-user-policy-card')

    // Enable the sandboxed-stdio transport → the flavor Select appears.
    await card.getByTestId('mcp-policy-transport-stdio').check()
    await expect(card.getByTestId('mcp-policy-flavor-select')).toBeVisible()

    // Saving with stdio allowed but NO flavor picked is rejected client-side —
    // handleSave returns early (toast only), so NO policy PUT is sent.
    let putFired = false
    page.on('response', r => {
      if (/\/api\/mcp\/user-policy$/.test(r.url()) && r.request().method() === 'PUT') {
        putFired = true
      }
    })
    await card.getByTestId('mcp-policy-save-btn').click()
    await page.waitForTimeout(1000)
    expect(putFired).toBe(false)
  })

  test('picking a flavor from the SandboxFlavors catalog saves the policy', async ({
    page,
  }) => {
    // Persisting a stdio user policy is rejected (422 MCP_SANDBOX_DISABLED) unless
    // code_sandbox is enabled in the deployment. E2E boots with code_sandbox OFF
    // by default (it needs bwrap + a mounted rootfs); the run opts in via
    // ZIEE_E2E_SANDBOX=1. Without that, this save path can't succeed, so skip —
    // matching the rootfs-gated sandbox specs. The sibling client-side test above
    // exercises the picker without needing the backend feature.
    test.skip(
      process.env.ZIEE_E2E_SANDBOX !== '1',
      'code_sandbox disabled in default E2E deployment — stdio policy save requires ZIEE_E2E_SANDBOX=1',
    )
    const card = page.getByTestId('mcp-user-policy-card')
    await card.getByTestId('mcp-policy-transport-stdio').check()

    // Open the flavor Select (options come from the SandboxFlavors store —
    // the admin /api/code-sandbox/flavors catalog, or the full/minimal
    // fallback) and choose the first one (derived -opt-<value> id).
    await card.getByTestId('mcp-policy-flavor-select').click()
    const option = page.getByTestId(/^mcp-policy-flavor-select-opt-/).first()
    await expect(option).toBeVisible()
    await option.click()

    // Saving persists the policy — assert the PUT round-trip succeeds.
    const saved = page.waitForResponse(
      r => /\/api\/mcp\/user-policy$/.test(r.url()) && r.request().method() === 'PUT',
    )
    await card.getByTestId('mcp-policy-save-btn').click()
    expect((await saved).status()).toBe(200)
  })
})
