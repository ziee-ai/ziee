import { gzipSync } from 'zlib'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the per-conversation ConversationSkillsPanel hide/unhide toggle
 * (Path B opt-out).
 *
 * Audit gap: the panel's per-conversation enable/disable Switch had no E2E.
 * This imports a skill, creates a conversation, opens the "Skills in this
 * chat" panel from the chat "+" dropdown, and toggles the skill OFF —
 * asserting the hide-in-conversation POST fires and the Switch flips.
 */

const SKILL_MD = `---
name: e2e-conv-skill
description: A skill used by the per-conversation panel E2E.
---

# E2E Conversation Skill

Body content.
`

/** Build a one-file (SKILL.md) tar.gz bundle. */
function buildSkillBundle(md: string): Buffer {
  const content = Buffer.from(md, 'utf8')
  const header = Buffer.alloc(512)
  header.write('SKILL.md', 0, 'utf8')
  header.write('0000644\0', 100, 'utf8')
  header.write('0000000\0', 108, 'utf8')
  header.write('0000000\0', 116, 'utf8')
  header.write(content.length.toString(8).padStart(11, '0') + '\0', 124, 'utf8')
  header.write('00000000000\0', 136, 'utf8')
  header.write('0', 156, 'utf8')
  header.write('ustar\0', 257, 'utf8')
  header.write('00', 263, 'utf8')
  for (let i = 148; i < 156; i++) header[i] = 0x20
  let sum = 0
  for (let i = 0; i < 512; i++) sum += header[i]
  header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'utf8')
  const bodyPad = (512 - (content.length % 512)) % 512
  return gzipSync(
    Buffer.concat([header, content, Buffer.alloc(bodyPad), Buffer.alloc(1024)]),
  )
}

test.describe('Skills — per-conversation panel toggle', () => {
  test('hiding a skill in a conversation fires the hide API and flips the switch', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Import a skill so the panel has a row.
    const slug = `convskill${Date.now().toString(36)}`
    const importRes = await request.post(
      `${apiURL}/api/skills/import?name=${slug}`,
      {
        headers: { Authorization: `Bearer ${token}` },
        multipart: {
          bundle: {
            name: 'bundle.tar.gz',
            mimeType: 'application/gzip',
            buffer: buildSkillBundle(SKILL_MD),
          },
        },
      },
    )
    expect(importRes.status(), `import: ${await importRes.text()}`).toBe(201)

    // Create a conversation (no model needed) and open it.
    const convRes = await request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Skill panel conv' },
    })
    expect(convRes.status()).toBeLessThan(300)
    const convId = (await convRes.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)

    // Open the "+" dropdown → "Skills in this chat".
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByRole('button', { name: 'Skills in this chat' }).click()

    // The panel lists the imported skill with a visibility Switch (on).
    const sw = page.getByRole('switch').first()
    await expect(sw).toBeVisible({ timeout: 15000 })
    await expect(sw).toBeChecked()

    // Toggle off → hide-in-conversation POST fires + the switch flips.
    const hideResp = page.waitForResponse(
      r =>
        /\/api\/skills\/[^/]+\/hide-in-conversation$/.test(r.url()) &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await sw.click()
    expect((await hideResp).status()).toBeLessThan(400)
    await expect(sw).not.toBeChecked()
  })
})
