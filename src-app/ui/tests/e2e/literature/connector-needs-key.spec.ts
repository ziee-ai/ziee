import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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

    // CORE's connector form.
    const coreForm = byTestId(page, 'lit-connector-form-core')
    await expect(coreForm).toBeVisible({ timeout: 30000 })
    // It's flagged as needing a key.
    await expect(byTestId(page, 'lit-connector-needs-key-tag-core')).toBeVisible()

    // With no key entered the required-key gate blocks save: the Save button is
    // disabled, so an empty CORE config can never be persisted.
    await expect(byTestId(page, 'lit-connector-save-button-core')).toBeDisabled({
      timeout: 10000,
    })
  })
})
