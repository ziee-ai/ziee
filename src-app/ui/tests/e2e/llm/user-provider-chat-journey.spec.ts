import { test, expect } from '../../fixtures/test-context'
import {
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'
import { createModelViaAPI } from '../../common/provider-helpers'
import {
  goToNewChatPage,
  waitForNewChatPageLoad,
  selectModelInDropdown,
  sendChatMessage,
  getLastMessageContent,
} from '../chat/helpers/chat-helpers'
import { byTestId } from '../testid'

/**
 * E2E — a USER-configured provider key is actually usable for a real chat
 * (audit gap all-e8a9d734fadf).
 *
 * The existing user-llm-providers spec proves the personal-key save/list/delete
 * UI works, and tests/llm_provider/mod.rs proves the REST surface — but nothing
 * proved the saved personal key is the credential that actually drives a real
 * model call. This stitches the whole combination together end-to-end.
 *
 * The honest trick that makes this a REAL test (not cosmetic): the admin
 * configures the Anthropic provider with a DELIBERATELY INVALID system key, and
 * the (non-admin) user saves their OWN valid key on the user-providers page.
 * The backend's key resolution is "user's personal key wins, falls back to
 * system" (chat/core/ai_provider/mod.rs::resolve_api_key_for_user). So a chat
 * that succeeds COULD ONLY have used the user's personal key — the system key
 * is invalid and would 401 upstream. If the personal-key path were broken, the
 * chat would fail, and this test would fail.
 *
 * Real-LLM, gated on ANTHROPIC_API_KEY (soft-skip when unset), Haiku model.
 */

const HAS_KEY = Boolean(process.env.ANTHROPIC_API_KEY)
const HAIKU_MODEL = 'claude-haiku-4-5-20251001'
const HAIKU_DISPLAY = `Haiku UserKey ${Date.now().toString(36)}`

test.describe('User LLM provider — personal key drives a real chat', () => {
  test('a user-saved Anthropic key chats successfully despite an invalid system key', async ({
    page,
    testInfra,
  }) => {
    test.skip(
      !HAS_KEY,
      'ANTHROPIC_API_KEY not set — skipping real-LLM user-key chat journey',
    )
    test.setTimeout(120_000)

    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const realKey = process.env.ANTHROPIC_API_KEY as string
    const tag = Date.now().toString(36)

    // 1) Anthropic provider with an INVALID system key. It is enabled (the
    //    backend requires a non-empty key to enable a remote provider), but
    //    that key is junk — only a valid USER key can make a call succeed.
    const providerName = `e2e-userkey-${tag}`
    const createResp = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({
        name: providerName,
        provider_type: 'anthropic',
        enabled: true,
        base_url: 'https://api.anthropic.com/v1',
        api_key: 'sk-ant-INVALID-system-key-must-not-work',
      }),
    })
    expect(
      createResp.ok,
      `create provider: ${createResp.status} ${await createResp.clone().text()}`,
    ).toBeTruthy()
    const provider = await createResp.json()

    // Assign the provider to the default group so the auto-joined user sees it.
    const groupsResp = await fetch(`${apiURL}/api/groups`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    const groupsBody = await groupsResp.json()
    const defaultGroup = (groupsBody.groups as Array<{ id: string; is_default?: boolean }>).find(
      g => g.is_default,
    )
    if (!defaultGroup) throw new Error('No default group found')
    const assignResp = await fetch(
      `${apiURL}/api/llm-providers/${provider.id}/groups`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${adminToken}`,
        },
        body: JSON.stringify({ group_id: defaultGroup.id }),
      },
    )
    expect(assignResp.ok, `assign provider to default group: ${assignResp.status}`).toBeTruthy()

    // 2) A Haiku model on that provider (enabled), selectable in chat.
    await createModelViaAPI(
      apiURL,
      adminToken,
      provider.id,
      HAIKU_MODEL,
      HAIKU_DISPLAY,
      'anthropic',
    )

    // 3) A brand-new NON-admin user (auto-joined to the default group).
    const uname = `userkey_${tag}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')

    // 4) The user saves THEIR OWN valid key on the user-providers page.
    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')
    await byTestId(page, `ullm-provider-menu-item-${provider.id}`).first().click()
    await byTestId(page, 'ullm-key-password-input').fill(realKey)
    const saveBtn = byTestId(page, 'ullm-save-key-button')
    await expect(saveBtn).toBeEnabled()
    await saveBtn.click()
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Your key configured', { timeout: 15_000 })

    // 5) The user chats with the Haiku model — success proves the personal
    //    key was used (the system key is invalid).
    await goToNewChatPage(page, baseURL)
    await waitForNewChatPageLoad(page)
    await selectModelInDropdown(page, HAIKU_DISPLAY)
    await sendChatMessage(page, 'Reply with exactly the single word: READY')

    const content = (await getLastMessageContent(page)).trim()
    expect(
      content.length,
      'assistant produced a non-empty reply using the user-configured key',
    ).toBeGreaterThan(0)
  })
})
