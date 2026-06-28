import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — project detail-page conversation LIST interaction.
 *
 * detail-page-layout.spec only asserts the empty state. This seeds an attached
 * conversation and asserts the conversations section renders the card (not the
 * empty state) and that clicking it navigates into that conversation's chat.
 */

async function seedProject(apiURL: string, token: string, name: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`seed project failed: ${res.status}`)
  return (await res.json()).id
}

async function seedConv(apiURL: string, token: string, title: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed conv failed: ${res.status}`)
  return (await res.json()).id
}

async function attach(apiURL: string, token: string, projectId: string, conversationId: string) {
  const res = await fetch(
    `${apiURL}/api/projects/${projectId}/conversations/${conversationId}`,
    { method: 'POST', headers: { Authorization: `Bearer ${token}` } },
  )
  if (!res.ok) throw new Error(`attach failed: ${res.status}`)
}

test.describe('Projects — conversation list interaction', () => {
  test('an attached conversation renders in the list and navigates on click', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, `ConvList ${Date.now()}`)
    const convTitle = `Project conv ${Date.now()}`
    const conversationId = await seedConv(apiURL, token, convTitle)
    await attach(apiURL, token, projectId, conversationId)

    await page.goto(`${baseURL}/projects/${projectId}`)

    const section = page.locator('[data-test-section="conversations"]')
    await expect(section).toBeVisible({ timeout: 30000 })
    // No longer the empty state — the seeded conversation card appears.
    await expect(
      section.getByText(/no conversations in this project yet/i),
    ).toHaveCount(0)
    const card = section.locator('.ant-card').filter({ hasText: convTitle })
    await expect(card).toBeVisible({ timeout: 15000 })

    // Clicking the card navigates into that conversation's chat.
    await card.click()
    await expect(page).toHaveURL(new RegExp(conversationId), { timeout: 15000 })
  })
})
