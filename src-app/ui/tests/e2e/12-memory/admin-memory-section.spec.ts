import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Memory admin master "Memory" card (MemorySection.tsx).
 *
 * Audit gap: the deployment-wide enable Switch + Default top-K + Save
 * (handleSubmit → Stores.MemoryAdmin.update) had no E2E. This toggles the
 * deployment-wide switch and saves, asserting the success toast — the real
 * store→PUT round-trip, not a render check.
 */

test.describe('Memory — admin master section', () => {
  test('toggle deployment-wide enable + Save shows the success toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory-admin`)

    const enableSwitch = page.getByRole('switch', {
      name: 'Enable memory deployment-wide',
    })
    await expect(enableSwitch).toBeVisible({ timeout: 30000 })

    // The Save button belongs to the card holding this switch.
    const memoryCard = page.locator(
      '.ant-card:has([aria-label="Enable memory deployment-wide"])',
    )
    await enableSwitch.click()
    await memoryCard.getByRole('button', { name: 'Save' }).click()

    await expect(page.getByText('Memory settings saved.')).toBeVisible({
      timeout: 30000,
    })
  })
})
