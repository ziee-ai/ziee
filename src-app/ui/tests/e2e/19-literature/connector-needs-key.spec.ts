import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — LitSearchConnectorsSection required-key validation.
 *
 * The CORE connector declares a REQUIRED api key; its config form's key field
 * carries a `{label} is required` rule when the key isn't set yet
 * (LitSearchConnectorsSection.tsx:195-198). Saving without entering a key must
 * surface that inline error and block the update.
 */

test.describe('Literature — connector needs-key validation', () => {
  test('saving the CORE connector without a key shows the required error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/literature`)

    // CORE's connector form (antd Form name → field id prefix).
    const coreForm = page.locator('form:has(#lit-connector-core_api_key)')
    await expect(coreForm).toBeVisible({ timeout: 30000 })
    // It's flagged as needing a key.
    await expect(page.getByText('CORE (open-access full text)')).toBeVisible()

    // Save with an empty key → inline required-field error, no success toast.
    await coreForm.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('CORE API key is required')).toBeVisible({
      timeout: 10000,
    })
    await expect(page.getByText(/CORE.*saved/)).toHaveCount(0)
  })
})
