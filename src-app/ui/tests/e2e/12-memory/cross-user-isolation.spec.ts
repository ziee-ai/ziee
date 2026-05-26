import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginAs,
} from '../../common/auth-helpers'

/**
 * E2E — cross-user isolation regression.
 *
 * Plan §10 mandatory regression: user A creates a memory; user B
 * logging in and visiting /memories must not see it. This guards the
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
    await loginAs(page, baseURL, alice, 'password123')
    await page.goto(`${baseURL}/memories`)
    await page.getByRole('button', { name: /Add memory/ }).click()
    await page
      .getByRole('dialog')
      .getByLabel('Content')
      .fill('Alice is a secret agent')
    await page.getByRole('dialog').getByRole('button', { name: /^Add$/ }).click()
    await expect(page.getByText('Memory added')).toBeVisible()
    await expect(page.getByText('Alice is a secret agent')).toBeVisible()

    // Log out and back in as Bob.
    await page.context().clearCookies()
    await page.goto(`${baseURL}/login`)
    await loginAs(page, baseURL, bob, 'password123')
    await page.goto(`${baseURL}/memories`)

    // Bob's list must NOT contain Alice's secret.
    await expect(page.getByText('Alice is a secret agent')).not.toBeVisible()
  })
})
