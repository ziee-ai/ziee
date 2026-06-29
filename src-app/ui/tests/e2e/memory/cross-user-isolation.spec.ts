import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — cross-user isolation regression.
 *
 * Plan §10 mandatory regression: user A creates a memory; user B
 * logging in and visiting /settings/memory must not see it. This guards the
 * SQL filter at the repository layer end-to-end through the UI.
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
    'memory::read',
    'memory::write',
  ])
  return username
}

test.describe('Memory — cross-user isolation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test("user B does not see user A's memories", async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const alice = await memoryUser(apiURL, 'iso_alice')
    const bob = await memoryUser(apiURL, 'iso_bob')

    // Alice logs in, adds a memory.
    await login(page, baseURL, alice, 'password123')
    await page.goto(`${baseURL}/settings/memory`)
    await byTestId(page, 'memory-add-btn').click()
    await byTestId(page, 'memory-create-content-input').fill('Alice is a secret agent')
    await byTestId(page, 'memory-create-submit-btn').click()
    // Seeded content is dynamic data this test created — assert inside the list card.
    await expect(byTestId(page, 'memory-my-card')).toContainText('Alice is a secret agent')

    // Log out and back in as Bob.
    await page.context().clearCookies()
    await page.goto(`${baseURL}/login`)
    await login(page, baseURL, bob, 'password123')
    await page.goto(`${baseURL}/settings/memory`)

    // Bob's list must NOT contain Alice's secret.
    await expect(byTestId(page, 'memory-my-card')).not.toContainText('Alice is a secret agent')
  })
})
