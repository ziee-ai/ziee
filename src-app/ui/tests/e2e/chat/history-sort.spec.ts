import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * ITEM-6 / TEST-12 — conversation history sort control.
 *
 * Seeds three conversations (REST) whose creation order differs from alpha
 * order, then drives the sort Select and asserts the visible list reorders
 * (server-side sort). Titles only — no messages needed for sort.
 */

async function seedConversation(
  apiURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status} ${await res.text()}`)
  return (await res.json()).id as string
}

/** The conversation ids in the order they appear in the list. */
async function visibleCardIds(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    Array.from(
      document.querySelectorAll('[data-testid^="chat-conversation-card-"]'),
    ).map(el =>
      (el.getAttribute('data-testid') || '').replace('chat-conversation-card-', ''),
    ),
  )
}

async function chooseSort(page: Page, label: string) {
  await page.getByTestId('chat-history-sort-select').click()
  await page.getByRole('option', { name: label }).click()
}

test.describe('Chat history — sort', () => {
  test('reorders the conversation list by the selected sort', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Creation order: Mango, Apple, Zebra → recent = Zebra,Apple,Mango;
    // oldest = Mango,Apple,Zebra; alpha = Apple,Mango,Zebra (all distinct).
    const mango = await seedConversation(apiURL, token, 'Mango notes')
    const apple = await seedConversation(apiURL, token, 'Apple notes')
    const zebra = await seedConversation(apiURL, token, 'Zebra notes')

    await page.goto(`${baseURL}/chats`)
    await expect(page.getByTestId(`chat-conversation-card-${mango}`)).toBeVisible({
      timeout: 30000,
    })

    // Default sort = recent (most-recently updated first).
    await expect
      .poll(() => visibleCardIds(page))
      .toEqual([zebra, apple, mango])

    // Alphabetical by title.
    await chooseSort(page, 'Title A–Z')
    await expect.poll(() => visibleCardIds(page)).toEqual([apple, mango, zebra])

    // Oldest first.
    await chooseSort(page, 'Oldest first')
    await expect.poll(() => visibleCardIds(page)).toEqual([mango, apple, zebra])
  })
})
