import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E for conversation deletion added in the hub cherry-pick:
 *   - per-card delete via a "Delete conversation?" Popconfirm (ConversationCard)
 *   - bulk select + "Delete Selected" via the selection bar (ConversationList)
 *
 * Conversations are seeded directly via POST /api/conversations (title only,
 * no model needed) so the tests stay fast and deterministic.
 */

async function seedConversation(
  apiURL: string,
  token: string,
  title: string,
): Promise<void> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) {
    throw new Error(
      `seedConversation(${title}) failed: ${res.status} ${await res.text()}`,
    )
  }
}

/** Locate the conversation Card that contains a given title. */
function cardByTitle(page: Page, title: string) {
  return page.locator('.ant-card').filter({ hasText: title })
}

async function gotoChats(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/chats`)
  // NOTE: do NOT wait for 'networkidle' here. The chat realtime-sync feature
  // opens two always-on SSE streams (chat tokens + sync) the moment a chat page
  // mounts; those long-lived requests never "finish", so Playwright's
  // networkidle never fires and the helper hangs until the test times out.
  // Wait for a concrete render signal instead — the seeded cards are asserted
  // visible by each caller right after.
  await page.waitForLoadState('domcontentloaded')
}

test.describe('Conversation deletion', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('delete a single conversation via the card Popconfirm', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedConversation(testInfra.apiURL, token, 'E2E Delete Me')
    await seedConversation(testInfra.apiURL, token, 'E2E Keep Me')

    await gotoChats(page, testInfra.baseURL)

    const target = cardByTitle(page, 'E2E Delete Me')
    await expect(target).toBeVisible({ timeout: 15000 })

    // Reveal + click the per-card delete (icon-only) button, then confirm.
    await target.hover()
    await target.locator('button:has(.anticon-delete)').click()

    const popconfirm = page.locator('.ant-popover').filter({ hasText: 'Delete conversation?' })
    await expect(popconfirm).toBeVisible()
    await popconfirm.getByRole('button', { name: 'Delete', exact: true }).click()

    await expect(cardByTitle(page, 'E2E Delete Me')).toHaveCount(0, { timeout: 10000 })
    // The other conversation is untouched.
    await expect(cardByTitle(page, 'E2E Keep Me')).toBeVisible()
  })

  test('bulk-delete selected conversations', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    await seedConversation(testInfra.apiURL, token, 'E2E Bulk One')
    await seedConversation(testInfra.apiURL, token, 'E2E Bulk Two')

    await gotoChats(page, testInfra.baseURL)

    const one = cardByTitle(page, 'E2E Bulk One')
    const two = cardByTitle(page, 'E2E Bulk Two')
    await expect(one).toBeVisible({ timeout: 15000 })
    await expect(two).toBeVisible()

    // Select both via their checkboxes; the first selection enters selection mode.
    await one.hover()
    await one.getByRole('checkbox').click()
    await two.hover()
    await two.getByRole('checkbox').click()

    // The bulk-action bar appears once something is selected.
    await expect(page.getByText(/2 conversations selected/i)).toBeVisible()

    await page.getByRole('button', { name: 'Delete Selected' }).click()
    const popconfirm = page
      .locator('.ant-popover')
      .filter({ hasText: 'Delete selected conversations' })
    await expect(popconfirm).toBeVisible()
    await popconfirm.getByRole('button', { name: 'Delete', exact: true }).click()

    await expect(cardByTitle(page, 'E2E Bulk One')).toHaveCount(0, { timeout: 10000 })
    await expect(cardByTitle(page, 'E2E Bulk Two')).toHaveCount(0)
  })
})
