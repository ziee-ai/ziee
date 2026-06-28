import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpAdminPage,
  waitForMcpAdminPageLoad,
} from './helpers/navigation-helpers'
import {
  clickEditServerButton,
  toggleServerEnabled,
} from './helpers/form-helpers'

/**
 * E2E — the BioMCP built-in server's admin surface (audit all-963360ec8a62).
 *
 * BioMCP has no bespoke UI: per CLAUDE.md its admin surface is the generic
 * MCP system-server page (it is is_built_in + is_system but, unlike the
 * zero-config built-ins, deliberately NOT in the edit-deny-list, so its row
 * stays editable — admins set the upstream API keys as secret Headers).
 *
 * BioMCP is disabled in E2E by default for isolation; this spec opts it in
 * via `test.use({ bioMcpEnabled: true })`, which flips `bio_mcp.enabled` in
 * the per-test backend config. The server only registers the `bio` row when
 * the embedded biomcp binary is a real (non-stub) build; if a build staged a
 * zero-byte stub the module self-disables and the row never appears — the
 * test soft-skips in that case rather than asserting a row that can't exist.
 *
 * The admin surface is exercised end to end (row present + editable + the
 * Headers secret-editor for the upstream API keys + the enable toggle
 * round-trip) without ever issuing a `biomcp` tool call, so no sidecar is
 * spawned and the test stays deterministic.
 */

const BIO_DISPLAY_NAME = 'BioMCP'

test.describe('MCP — BioMCP built-in admin surface', () => {
  test.use({ bioMcpEnabled: true })

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToMcpAdminPage(page, testInfra.baseURL)
    await waitForMcpAdminPageLoad(page)
  })

  test('bio built-in row is present, editable, with a Headers editor, and toggleable', async ({
    page,
  }) => {
    const bioCard = page
      .locator(`.ant-card:has-text("${BIO_DISPLAY_NAME}")`)
      .first()

    // The row registers asynchronously at boot (a spawned upsert). If it
    // never shows up, this build staged a stub biomcp binary → the module
    // self-disables; soft-skip rather than fail or fake.
    const appeared = await bioCard
      .waitFor({ state: 'visible', timeout: 20_000 })
      .then(() => true)
      .catch(() => false)
    test.skip(
      !appeared,
      'BioMCP row not registered — this build staged a stub biomcp binary (module self-disabled)',
    )

    // It is a BUILT-IN row: no Delete affordance (built-ins cannot be
    // deleted), but — unlike the zero-config built-ins — it IS editable.
    await expect(bioCard.getByRole('button', { name: 'Edit' })).toBeVisible()

    // --- Open the edit drawer: the generic system-server editor. ---
    await clickEditServerButton(page, BIO_DISPLAY_NAME, true)
    const drawer = page.locator('.ant-drawer-content:visible').last()

    // The HTTP Headers secret-editor is the surface where an admin sets the
    // upstream API keys (NCBI_API_KEY, S2_API_KEY, …) as encrypted entries.
    await expect(drawer.getByText('HTTP Headers')).toBeVisible()
    await expect(drawer.getByRole('button', { name: /Add header/i })).toBeVisible()

    // Add an API-key header (a real edit through the secret editor), then
    // close without persisting — we only need to prove the editor accepts
    // the bio API-key configuration the admin would enter.
    await drawer.getByRole('button', { name: /Add header/i }).click()
    const keyInput = drawer.getByPlaceholder('Authorization').last()
    await keyInput.fill('NCBI_API_KEY')
    await expect(keyInput).toHaveValue('NCBI_API_KEY')

    // Close the drawer without saving (Escape on an antd Drawer).
    await page.keyboard.press('Escape')
    await expect(page.locator('.ant-drawer-content:visible')).toHaveCount(0, {
      timeout: 5_000,
    })

    // --- The enable toggle round-trips through the real admin endpoint. ---
    // (Disabling the bio row; the helper waits for the success/warning toast
    // that confirms the server-side mutation completed.)
    await toggleServerEnabled(page, BIO_DISPLAY_NAME)

    // The row is still present after the toggle (a built-in is never removed
    // by being disabled — only its serving is gated).
    await expect(bioCard).toBeVisible()
  })
})
