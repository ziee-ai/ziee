import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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

    const enableSwitch = byTestId(page, 'memory-admin-enabled-switch')
    await expect(enableSwitch).toBeVisible({ timeout: 30000 })

    await enableSwitch.click()
    await byTestId(page, 'memory-admin-master-save-btn').click()

    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Memory settings saved.',
      { timeout: 30000 },
    )
  })
})
