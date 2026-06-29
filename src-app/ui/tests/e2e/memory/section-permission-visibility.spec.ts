import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'
import { byTestId } from '../testid'

/**
 * E2E — permission-gated section visibility on /settings/memory.
 *
 * The route is gated `anyOf(memory::read, memory::core::read)`, so a user with
 * ONLY `memory::core::read` can open the page, but the `memory::read`-gated
 * sections (`MyMemoriesSection` "My memories", `PreferencesSection`
 * "Preferences") return null while the core-memory section stays visible.
 */

test.describe('Memory — section permission visibility', () => {
  test('a core-read-only user sees the core section but not the memory::read sections', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // memory::core::read grants route access; memory::read is withheld.
    await loginWithPerms(page, baseURL, apiURL, [Permissions.CoreMemoryRead])

    await page.goto(`${baseURL}/settings/memory`)

    // Positive control: the core-memory section (gated on core::read) renders.
    await expect(byTestId(page, 'memory-core-card')).toBeVisible({
      timeout: 30000,
    })

    // The memory::read-gated sections are NOT rendered.
    await expect(byTestId(page, 'memory-my-card')).toHaveCount(0)
    await expect(byTestId(page, 'memory-prefs-card')).toHaveCount(0)
  })
})
