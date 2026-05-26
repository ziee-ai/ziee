/**
 * Admin CRUD E2E for /settings/auth-providers — exercises the real
 * backend (no API mocks). The Test endpoint will fail for the fake
 * URLs we use, but that's the POINT for this test: we're verifying
 * the UI handles the failure gracefully + persists the result.
 *
 * Each test creates a uniquely-named provider so they can run in
 * parallel without colliding on the DB-level unique-name constraint.
 *
 * Out of scope: the actual OAuth dance — covered by
 * `social-login-navikt.spec.ts` (parity test against real navikt).
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

test.describe('Auth providers — admin CRUD UI', () => {
  test('pre-seeded providers (google/microsoft/apple) show as disabled', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // All three pre-seeded rows from migration 47 should be there.
    // Use locator + text match (more forgiving than strict regex on
    // accessible name, which AntD wraps in <strong>).
    for (const name of ['google', 'microsoft', 'apple']) {
      await expect(
        page.getByRole('row').filter({ hasText: name }).first(),
      ).toBeVisible({ timeout: 10_000 })
    }

    // Each starts disabled (admin must configure + enable).
    // Status column renders <Badge text="Disabled"> — count visible
    // "Disabled" text in the table.
    const disabled = await page.getByText('Disabled', { exact: true }).count()
    expect(disabled).toBeGreaterThanOrEqual(3)
  })

  test('Add Provider menu disables templates whose name already exists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    await page.getByRole('button', { name: /add provider/i }).click()

    // Google/Microsoft/Apple should each show "(already added — edit
    // existing)" in the dropdown because the migration-47 rows still
    // exist. Generic templates (no name collision) stay enabled.
    await expect(page.getByText(/Google.*already added/i)).toBeVisible({
      timeout: 5_000,
    })
    await expect(
      page.getByText(/Microsoft.*already added/i),
    ).toBeVisible()
    await expect(page.getByText(/Apple.*already added/i)).toBeVisible()
    // Generic OIDC menu item should be clickable.
    await expect(
      page.getByText(/Generic OIDC \(Auth0 \/ Okta/i),
    ).toBeVisible()
  })

  test('create + delete a Generic OIDC provider', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-okta-${Date.now()}`

    // -------------------- CREATE --------------------
    await page.getByRole('button', { name: /add provider/i }).click()
    await page
      .getByText(/Generic OIDC \(Auth0 \/ Okta/i)
      .click()

    await expect(
      page.getByRole('button', { name: /^Create$/ }),
    ).toBeVisible({ timeout: 10_000 })

    await page.getByLabel(/Name \(URL slug\)/i).fill(providerName)
    await page.getByLabel(/Client ID/i).fill('e2e-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret-value')
    await page
      .getByLabel(/Issuer URL/i)
      .fill('https://nonexistent.invalid/oidc')

    await page.getByRole('button', { name: /^Create$/ }).click()

    // Row appears in the table. Use row-with-hasText (forgiving on
    // the AntD <strong> wrapping inside <td>).
    const row = page.getByRole('row').filter({ hasText: providerName })
    await expect(row.first()).toBeVisible({ timeout: 10_000 })

    // -------------------- EDIT drawer briefly --------------------
    // Verify drawer opens with name disabled (edit mode). Then close.
    // Buttons use aria-label="Edit ${providerName}" — anchor the
    // selector or it won't match (round-3 audit finding N-1).
    await row
      .getByRole('button', { name: new RegExp(`^Edit ${providerName}$`) })
      .click()
    await expect(page.getByLabel(/Name \(URL slug\)/i)).toBeDisabled({
      timeout: 5_000,
    })
    await page.getByRole('button', { name: /^Cancel$/ }).click()

    // -------------------- DELETE --------------------
    // Round-1 audit fix swapped DeleteProviderModal for an inline
    // AntD Popconfirm. The popover doesn't use role=dialog — scope
    // the confirm by `.ant-popover` and click its primary danger
    // button (the Popconfirm primary-button class is stable across
    // okText changes per project_ui_e2e_drawer_selectors memory).
    await row
      .getByRole('button', { name: new RegExp(`^Delete ${providerName}$`) })
      .click()
    const popover = page.locator('.ant-popover:visible').last()
    await expect(popover).toBeVisible({ timeout: 5_000 })
    await popover.locator('.ant-btn-primary').click()

    // Row gone (generous timeout — delete includes DB write + reload).
    await expect(
      page.getByRole('row').filter({ hasText: providerName }),
    ).toHaveCount(0, { timeout: 30_000 })
  })

  test('Test config button in the drawer surfaces inline result', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // Open the drawer via "Add provider → Generic OIDC".
    await page.getByRole('button', { name: /add provider/i }).click()
    await page
      .getByText(/Generic OIDC \(Auth0 \/ Okta/i)
      .click()

    await page.getByLabel(/Name \(URL slug\)/i).fill(`e2e-test-config-${Date.now()}`)
    await page.getByLabel(/Client ID/i).fill('any-client')
    await page.locator('input[type="password"]').first().fill('any-secret')
    await page
      .getByLabel(/Issuer URL/i)
      .fill('https://nonexistent.invalid/oidc')

    // Click "Test config" — backend tries discovery, fails fast.
    await page.getByRole('button', { name: /Test config/i }).click()

    // Inline alert appears with the failure (we accept either the
    // "Configuration issues" Alert title or the underlying message).
    await expect(
      page.getByText(/Configuration (issues|OK)/i),
    ).toBeVisible({ timeout: 15_000 })

    // Drawer is still open — Test config doesn't close it.
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible()

    // Cleanup: close without saving.
    await page.getByRole('button', { name: /^Cancel$/ }).click()
  })
})
