import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the "My memories" list empty state (MyMemoriesSection.tsx).
 *
 * Audit gap: manual-add.spec adds a memory then lists it, but the
 * zero-memories empty state (`<Empty description="No memories yet" />`)
 * was never asserted. A fresh admin has no memories, so visiting
 * /settings/memory must render the empty state.
 */

test.describe('Memory — list empty state', () => {
  test('a user with no memories sees the empty state', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory`)

    await expect(byTestId(page, 'memory-empty')).toBeVisible({
      timeout: 30000,
    })
  })
})
