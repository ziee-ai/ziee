import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
    const card = byTestId(page, 'memory-prefs-card')
    await expect(card).toBeVisible({ timeout: 15_000 })

    // The extraction toggle is the "Auto-extract memories" row's switch.
    const extractionSwitch = byTestId(card, 'memory-prefs-extraction-switch')
    await expect(extractionSwitch).toBeVisible({ timeout: 15_000 })

    // Read the current checked state so we can assert it actually flipped.
    const wasChecked =
      (await extractionSwitch.getAttribute('aria-checked')) === 'true'

    // A unique cap value so the asserted PUT/persistence can't be a stale read.
    const newCap = 4000 + (Date.now() % 5000)
    const capInput = byTestId(card, 'memory-prefs-max-input')
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
    await byTestId(card, 'memory-prefs-save-btn').click()
    const put = await putPromise
    expect(put.status()).toBe(200)

    const body = JSON.parse(put.request().postData() ?? '{}')
    expect(body.extraction_enabled).toBe(!wasChecked)
    expect(body.max_memories).toBe(newCap)

    // Persistence: a fresh load re-hydrates the saved values from the server.
    await page.reload()
    const reloadedCard = byTestId(page, 'memory-prefs-card')
    const reloadedSwitch = byTestId(
      reloadedCard,
      'memory-prefs-extraction-switch',
    )
    await expect(reloadedSwitch).toHaveAttribute(
      'aria-checked',
      String(!wasChecked),
      { timeout: 15_000 },
    )
    const reloadedCap = byTestId(reloadedCard, 'memory-prefs-max-input')
    await expect(reloadedCap).toHaveValue(String(newCap), { timeout: 15_000 })
  })
})
