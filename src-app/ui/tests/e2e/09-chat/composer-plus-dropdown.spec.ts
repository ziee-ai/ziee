import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

// audit id all-aed242b6d0f2 — the chat composer "+" (Add attachment) dropdown
// aggregates slot-contributed menu items (file attach, MCP tools, skills,
// assistant). Its rendering was untested. This opens the dropdown and asserts
// the file + MCP items render. (The skills item is conversation-gated and is
// covered by 16-skills/chat-skill-opt-out.spec.ts.)
test.describe('Chat composer — "+" dropdown', () => {
  test('opens and shows the file + MCP menu items', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    // A local provider+model needs no API key but makes the composer usable.
    const providerId = await createProviderViaAPI(apiURL, token, 'Local', 'local')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'local')

    await goToNewChatPage(page, baseURL)

    await page.getByRole('button', { name: 'Add attachment' }).click()

    await expect(page.getByText('Attach files or photos')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('MCP tools & servers')).toBeVisible()

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
    await page.getByRole('button', { name: 'Add attachment' }).first().click()

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
