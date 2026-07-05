import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the chat composer "+" dropdown menu (ChatInput.tsx toolbar_plus_items
 * slot). The dropdown aggregates extension menu items: file attach
 * (FileAttachMenuItem), per-conversation skills (SkillMenuItem — only when a
 * conversation id exists), and MCP config (McpMenuItem — when ≥1 enabled
 * server). Their combined rendering + the MCP item's interaction were untested.
 */

test.describe('Chat — composer "+" dropdown', () => {
  test('the dropdown lists file/skill/MCP items and MCP opens the config modal', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // SkillMenuItem only renders for an EXISTING conversation (needs an id), so
    // seed one and open it.
    const convRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title: 'plus-dropdown-conv' }),
    })
    expect(convRes.ok).toBeTruthy()
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', { timeout: 30000 })

    // Open the "+" dropdown.
    await byTestId(page, 'chat-input-add-btn').first().click()

    // All three extension menu items render.
    await expect(page.getByText('Attach files or photos')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Skills in this chat')).toBeVisible()
    await expect(page.getByText('MCP tools & servers')).toBeVisible()

    // Interaction: clicking the MCP item opens the MCP Configuration modal.
    await page.getByText('MCP tools & servers').click()
    await expect(
      page.getByRole('dialog').filter({ hasText: 'MCP Configuration' }),
    ).toBeVisible({ timeout: 10000 })
  })
})
