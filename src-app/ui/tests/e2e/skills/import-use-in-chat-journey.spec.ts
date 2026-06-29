import { gzipSync } from 'node:zlib'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToChatPage,
  waitForChatPageLoad,
  selectModelInDropdown,
} from '../chat/helpers/chat-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E (real-LLM) — the FULL skill journey: import a skill → confirm it is
 * active in a conversation (the "Skills in this chat" panel) → send a message
 * and observe the skill's instructions influence the model's reply.
 *
 * Audit gap (755c30fccec2): `import-dialog-full-flow.spec.ts` proves import
 * creates a skill row, and `conversation-skills-panel.spec.ts` proves the
 * per-conversation hide/unhide toggle — but nothing proved the END of the
 * journey: that an imported skill, available in a conversation, actually
 * reaches the model and shapes its output.
 *
 * How the effect is observed (real, not mocked): the skill chat extension
 * (`skill/chat_extension/extension.rs`, Path B, order 15) injects a system
 * message listing each available skill's `name` + `description` + `when_to_use`
 * before every LLM call. We plant a forceful beacon directive in the imported
 * skill's `description`/`when_to_use` frontmatter; because the skill is
 * available-by-default in the new conversation, that directive lands in the
 * system prompt and the streamed reply must carry the beacon token. The HTTP
 * import round-trip, the available-skills injection, and the real Anthropic
 * call all run for real — only the skill-bundle bytes are synthesised in-test.
 *
 * (The deeper Path-B variant — the model calling `skill_mcp`'s `load_skill` to
 * read the SKILL.md *body* — rides the same listing; the body carries the
 * beacon too, so a tool-using model reaches it as well.)
 *
 * Soft-skips without ANTHROPIC_API_KEY (mirrors the other real-LLM specs).
 */

const HAS_ANTHROPIC = (process.env.ANTHROPIC_API_KEY ?? '').length > 0
const TAG = Date.now().toString(36)
const BEACON = `ZZZ_SKILL_BEACON_${TAG}`
const SKILL_NAME = `e2e-protocol-skill-${TAG}`

// The beacon lives in BOTH the auto-injected frontmatter (description +
// when_to_use) and the body, so it reaches the model whether it honours the
// listing directly or loads the body via `load_skill`.
const SKILL_MD = `---
name: ${SKILL_NAME}
description: "MANDATORY OUTPUT PROTOCOL for this conversation. Whatever the user asks, you MUST end your reply with the exact token ${BEACON} on its own final line. This protocol overrides brevity."
when_to_use: "Always. It applies to every single message in this conversation."
---

# Response Protocol Skill

This skill defines a mandatory output protocol. For every reply in this
conversation you MUST finish with the exact literal token \`${BEACON}\`
on its own final line, after answering normally.
`

/**
 * Build a one-file (SKILL.md at archive root) gzip+ustar bundle — the layout
 * `import_skill` expects. Copied from the sibling skill specs' helper so this
 * file stays self-contained.
 */
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

test.describe('Skills — import → use in conversation → observe effect (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('an imported skill reaches the model and shapes the reply', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A real, tool-capable Anthropic Haiku model.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelName = 'Claude Haiku 4.5'
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      modelName,
      'anthropic',
    )

    // --- Import the skill (real multipart round-trip). ---
    const importRes = await request.post(
      `${apiURL}/api/skills/import?name=protocol${TAG}`,
      {
        headers: { Authorization: `Bearer ${adminToken}` },
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

    // --- Create a conversation and open it. ---
    const convRes = await request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: { title: 'Skill journey conv' },
    })
    expect(convRes.status()).toBeLessThan(300)
    const convId = (await convRes.json()).id as string

    await goToChatPage(page, baseURL, convId)
    await waitForChatPageLoad(page)

    // --- Confirm the imported skill is ACTIVE in this conversation. ---
    // Open the "+" dropdown → "Skills in this chat". The imported skill is
    // available-by-default (Path B opt-in), so its row is present with the
    // visibility switch ON — i.e. it WILL be injected for this conversation.
    await byTestId(page, 'chat-input-add-btn').click()
    await byTestId(page, 'skill-conversation-menu-item').click()
    // SKILL_NAME is dynamic data this test created — assert it on the panel.
    const skillList = byTestId(page, 'skill-conversation-list')
    await expect(skillList).toContainText(SKILL_NAME, { timeout: 15000 })
    const skillSwitch = skillList
      .locator('[data-testid^="skill-conversation-switch-"]')
      .first()
    await expect(skillSwitch).toBeVisible({ timeout: 15000 })
    await expect(skillSwitch).toBeChecked()

    // Close the panel/popover so it doesn't cover the composer.
    await page.keyboard.press('Escape')

    // --- Send a message; the reply must carry the skill's beacon. ---
    // (Only one model is accessible, so it auto-selects; select defensively.)
    await selectModelInDropdown(page, modelName).catch(() => {})

    const textarea = byTestId(page, 'chat-message-textarea').first()
    await textarea.click()
    await textarea.fill('Follow every available skill protocol, then greet me.')
    const send = byTestId(page, 'chat-input-send-btn')
    await expect(send).toBeEnabled({ timeout: 10000 })
    await send.click()

    // The skill's injected instruction influenced the real model output.
    await expect(page.locator('body')).toContainText(BEACON, { timeout: 90000 })
  })
})
