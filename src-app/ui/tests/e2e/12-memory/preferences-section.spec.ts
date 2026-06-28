import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the memory PreferencesSection standalone Save flow.
 *
 * `auto-extract.spec.ts` (and the other 12-memory specs) drive
 * `PUT /api/memory/settings` directly via `page.request`; none exercises
 * the PreferencesSection FORM — toggling the extraction/retrieval switches
 * + the storage-cap InputNumber and clicking the section's own Save button.
 * This covers that standalone save flow end-to-end through the real UI:
 * the form's `handleSubmit` → `Stores.MemorySettings.update` →
 * `PUT /api/memory/settings`, the success toast, and persistence on reload.
 */
test.describe('Memory — Preferences section save flow', () => {
  test('toggle extraction + change max-memories cap, Save persists via PUT', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory`)

    // The Preferences card (its own form + Save button).
    const card = page
      .locator('.ant-card')
      .filter({ has: page.getByText('Preferences', { exact: true }) })
    await expect(card).toBeVisible({ timeout: 15_000 })

    // The two switches live in labelled form rows; the extraction switch is
    // the "Auto-extract memories" row's switch.
    const extractionRow = card
      .locator('.ant-form-item')
      .filter({ hasText: 'Auto-extract memories' })
    const extractionSwitch = extractionRow.getByRole('switch')
    await expect(extractionSwitch).toBeVisible({ timeout: 15_000 })

    // Read the current checked state so we can assert it actually flipped.
    const wasChecked =
      (await extractionSwitch.getAttribute('aria-checked')) === 'true'

    // A unique cap value so the asserted PUT/persistence can't be a stale read.
    const newCap = 4000 + (Date.now() % 5000)
    const capInput = card
      .locator('.ant-form-item')
      .filter({ hasText: 'Max memories stored' })
      .getByRole('spinbutton')
    await capInput.fill(String(newCap))

    // Flip the extraction toggle.
    await extractionSwitch.click()
    await expect(extractionSwitch).toHaveAttribute(
      'aria-checked',
      String(!wasChecked),
    )

    // Save → assert the real PUT fires carrying the changed values.
    const putPromise = page.waitForResponse(
      r =>
        r.url().includes('/api/memory/settings') &&
        r.request().method() === 'PUT',
    )
    await card.getByRole('button', { name: 'Save' }).click()
    const put = await putPromise
    expect(put.status()).toBe(200)

    const body = JSON.parse(put.request().postData() ?? '{}')
    expect(body.extraction_enabled).toBe(!wasChecked)
    expect(body.max_memories).toBe(newCap)

    // The standalone success toast.
    await expect(page.getByText('Preferences saved.')).toBeVisible({
      timeout: 10_000,
    })

    // Persistence: a fresh load re-hydrates the saved values from the server.
    await page.reload()
    const reloadedCard = page
      .locator('.ant-card')
      .filter({ has: page.getByText('Preferences', { exact: true }) })
    const reloadedSwitch = reloadedCard
      .locator('.ant-form-item')
      .filter({ hasText: 'Auto-extract memories' })
      .getByRole('switch')
    await expect(reloadedSwitch).toHaveAttribute(
      'aria-checked',
      String(!wasChecked),
      { timeout: 15_000 },
    )
    const reloadedCap = reloadedCard
      .locator('.ant-form-item')
      .filter({ hasText: 'Max memories stored' })
      .getByRole('spinbutton')
    await expect(reloadedCap).toHaveValue(String(newCap), { timeout: 15_000 })
  })
})
