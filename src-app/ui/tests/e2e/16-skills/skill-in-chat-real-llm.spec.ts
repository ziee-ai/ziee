import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../09-chat/helpers/chat-helpers'

/**
 * E2E — a user installs a skill and the model LOADS it in chat via skill_mcp's
 * load_skill tool. The 16-skills specs only cover the admin/list pages; the
 * actual in-chat skill-loading flow (the point of the feature) was untested.
 * Real-LLM gated.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)
const SEED_SKILL_HUB_ID = 'io.github.ziee/effective-prompting'

test.describe('Skills — load/use in chat (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM skill-in-chat E2E skipped')

  test('the model calls load_skill when a request matches an installed skill', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Install the seeded hub skill so the skill chat-extension attaches skill_mcp.
    const inst = await fetch(`${apiURL}/api/skills/install-from-hub`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ hub_id: SEED_SKILL_HUB_ID }),
    })
    expect(inst.ok).toBeTruthy()

    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await sendChatMessage(
      page,
      'Use the load_skill tool to open the available skill, then briefly summarize what it teaches. You MUST call load_skill.',
    )

    // The skill_mcp load_skill tool call surfaces in the chat transcript.
    await expect(page.getByText(/load_skill/i).first()).toBeVisible({ timeout: 90_000 })
  })
})
