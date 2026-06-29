import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'

/**
 * E2E — SetupPage password validation surfaced through the UI.
 *
 * `SetupPage` enforces two client-side rules before the create-admin PUT fires:
 *   - password < 8 chars → "Password must be at least 8 characters"
 *   - confirm ≠ password → "Passwords do not match"
 * The onboarding-wizard spec only exercises the happy path, so these inline
 * validation errors were untested. This drives both via the real form.
 */

test.describe('Setup — admin password validation', () => {
  test('rejects a short password and a mismatched confirmation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })
    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')

    // Too-short password → inline length error, no navigation.
    await byTestId(page, 'app-setup-password-input').fill('short')
    await byTestId(page, 'app-setup-confirm-password-input').fill('short')
    await byTestId(page, 'app-setup-submit-button').click()
    await expect(
      byTestId(page, 'field-error-password'),
    ).toBeVisible({ timeout: 10000 })

    // Valid-length password but a mismatched confirmation → mismatch error.
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password999')
    await byTestId(page, 'app-setup-submit-button').click()
    await expect(byTestId(page, 'field-error-confirm_password')).toBeVisible({
      timeout: 10000,
    })

    // Still on /setup — neither invalid attempt created the admin.
    await expect(page).toHaveURL(/\/setup/)
  })
})
