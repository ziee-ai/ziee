import { gzipSync } from 'node:zlib'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'
import { byTestId } from '../testid.ts'

/**
 * Journey: import a skill (dev bundle) → it appears in the library → it is
 * available to USE inside a conversation (listed in the per-conversation
 * "Skills in this conversation" panel). The model-side effect of loading a
 * skill is covered by the backend skill_mcp tests; this proves the UI journey
 * from import to conversation availability.
 */

// A minimal single-file SKILL.md bundle (ustar tar + gzip), mirroring the
// workflow bundle builder. Frontmatter `name` becomes the display name;
// `description` is required by the importer.
function buildSkillBundle(skillMd: string): Buffer {
  const content = Buffer.from(skillMd, 'utf8')
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
  const tar = Buffer.concat([
    header,
    content,
    Buffer.alloc(bodyPad),
    Buffer.alloc(1024),
  ])
  return gzipSync(tar)
}

const SKILL_NAME = 'E2E Journey Skill'
const SKILL_MD = `---
name: ${SKILL_NAME}
description: Demonstrates the import-to-conversation journey for the E2E suite.
---

# ${SKILL_NAME}

When asked, follow this skill's guidance for the E2E journey test.
`

test.describe('Skills - import → use-in-conversation journey', () => {
  test('imported skill appears in the library and the conversation panel', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // 1. Import the skill via the dev bundle endpoint.
    const importResp = await request.post(
      `${apiURL}/api/skills/import?name=${encodeURIComponent('e2e-journey-skill')}`,
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
    expect(
      importResp.status(),
      `skill import should 201: ${await importResp.text()}`,
    ).toBe(201)

    // 2. It shows up in the user skills library.
    // SKILL_NAME is dynamic data this test created — a text filter on the
    // skill card is allowed.
    await goToSkillsPage(page, baseURL)
    await expect(
      page
        .locator('[data-testid^="skill-list-card-"]')
        .filter({ hasText: SKILL_NAME })
        .first(),
    ).toBeVisible({ timeout: 15000 })

    // 3. It is available to USE inside a conversation. Seed a model + a
    //    conversation, open the chat, then the per-conversation skills panel.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      undefined,
      'openai',
    )
    const conv = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: { title: 'skill-journey', model_id: modelId },
    })
    const conversationId: string = (await conv.json()).id

    await page.goto(`${baseURL}/chat/${conversationId}`)

    // Open the "+" dropdown → "Skills in this chat".
    await byTestId(page, 'chat-input-add-btn').click()
    await byTestId(page, 'skill-conversation-menu-item').click()

    // The modal lists every available skill, including the imported one
    // (SKILL_NAME is dynamic data this test created).
    const modal = byTestId(page, 'skill-conversation-dialog')
    await expect(modal).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'skill-conversation-list')).toContainText(
      SKILL_NAME,
      { timeout: 10000 },
    )
  })
})
