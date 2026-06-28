import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — per-conversation skills panel (the chat composer "+" → "Skills in this
 * chat" entry → `ConversationSkillsPanel`). The skill chat-extension adds the
 * opt-out entry; the panel lists installed skills with a per-skill visibility
 * Switch (hide/unhide in this conversation). Untested before.
 *
 * Installs the seeded hub skill, opens a conversation, opens the panel, and
 * toggles a skill's visibility Switch.
 */

const SEED_SKILL_HUB_ID = 'io.github.ziee/effective-prompting'

test.describe('Skills — per-conversation panel', () => {
  test('opening the panel lists a skill and toggling its Switch works', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Install the seeded hub skill for this user so the panel has a row.
    const inst = await fetch(`${apiURL}/api/skills/install-from-hub`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ hub_id: SEED_SKILL_HUB_ID }),
    })
    expect(inst.ok).toBeTruthy()

    // A conversation to scope the panel to.
    const convRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ title: 'skills-panel-conv' }),
    })
    expect(convRes.ok).toBeTruthy()
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', {
      timeout: 30000,
    })

    // Open the composer "+" dropdown and the "Skills in this chat" entry.
    await page.getByRole('button', { name: 'Add attachment' }).first().click()
    await page.getByText('Skills in this chat').first().click()

    // The per-conversation skills panel opens.
    const dialog = page.getByRole('dialog', {
      name: 'Skills in this conversation',
    })
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // The installed skill is listed with a visibility Switch — toggle it.
    const toggle = dialog.locator('.ant-switch').first()
    await expect(toggle).toBeVisible({ timeout: 10000 })
    const before = await toggle.getAttribute('aria-checked')
    await toggle.click()
    await expect(toggle).toHaveAttribute(
      'aria-checked',
      before === 'true' ? 'false' : 'true',
      { timeout: 10000 },
    )
  })
})
