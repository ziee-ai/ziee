import { test, expect } from '../../fixtures/test-context'

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
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')

    // Too-short password → inline length error, no navigation.
    await page.getByLabel('Password', { exact: true }).fill('short')
    await page.getByLabel('Confirm Password').fill('short')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(
      page.getByText('Password must be at least 8 characters'),
    ).toBeVisible({ timeout: 10000 })

    // Valid-length password but a mismatched confirmation → mismatch error.
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password999')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page.getByText('Passwords do not match')).toBeVisible({
      timeout: 10000,
    })

    // Still on /setup — neither invalid attempt created the admin.
    await expect(page).toHaveURL(/\/setup/)
  })
})
