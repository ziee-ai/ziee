import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createModelViaAPI } from '../../common/provider-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'
import { byTestId } from '../testid'

/**
 * E2E (real-LLM) — full journey: select a keyless model in chat → the API-key
 * modal prompts → enter a real key → Save & Select → SEND a message → a real
 * response streams back.
 *
 * Audit gap: provider-api-key-modal.spec stops at the modal save; this carries
 * the journey through to a real streamed response. Soft-skips without
 * ANTHROPIC_API_KEY.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

async function assignToAdminGroup(apiURL: string, auth: Record<string, string>, providerId: string) {
  const groups = await (await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })).json()
  const arr = Array.isArray(groups) ? groups : groups.groups || []
  const admin = arr.find((g: { name: string }) => g.name === 'Administrators')
  await fetch(`${apiURL}/api/groups/${admin.id}/providers`, {
    method: 'PUT',
    headers: auth,
    body: JSON.stringify({ provider_ids: [providerId] }),
  })
}

test.describe('LLM — model key-modal then send (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('enter the key in the modal, then send and get a response', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // A KEYED openai model (default selection, no modal) so the picker bootstraps.
    const keyedId = (await (await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST', headers: auth,
      body: JSON.stringify({ name: 'AAA Keyed', provider_type: 'openai', enabled: true, api_key: 'sk-keyed' }),
    })).json()).id
    await createModelViaAPI(apiURL, token, keyedId, 'keyed-model', 'Keyed Model', 'openai')
    await assignToAdminGroup(apiURL, auth, keyedId)

    // A KEYLESS anthropic provider + Haiku model → selecting it triggers the modal.
    const anthRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST', headers: auth,
      body: JSON.stringify({ name: 'ZZZ Anthropic Keyless', provider_type: 'anthropic', enabled: true }),
    })
    if (!anthRes.ok) throw new Error(`anthropic provider create failed: ${anthRes.status} ${await anthRes.text()}`)
    const anthId = (await anthRes.json()).id
    await createModelViaAPI(apiURL, token, anthId, 'claude-haiku-4-5-20251001', 'Haiku Keyless', 'anthropic')
    await assignToAdminGroup(apiURL, auth, anthId)

    await goToNewChatPage(page, baseURL)

    // Switch to the keyless Haiku model → the API-key modal appears.
    await byTestId(page, 'ullm-model-select').click()
    await page
      .locator('[data-testid^="ullm-model-select-opt-"]')
      .filter({ hasText: 'Haiku Keyless' })
      .first()
      .click()
    const modal = byTestId(page, 'ullm-apikey-dialog')
    await expect(modal).toBeVisible({ timeout: 10000 })

    // Enter the REAL key → Save & Select.
    await byTestId(page, 'ullm-apikey-password-input').fill(ANTHROPIC_KEY)
    await byTestId(page, 'ullm-apikey-save-button').click()
    await expect(modal).toBeHidden({ timeout: 10000 })

    // Send a message → a real assistant response streams back.
    await byTestId(page, 'chat-message-textarea').fill('Reply with the single word: pong')
    const send = byTestId(page, 'chat-input-send-btn')
    await expect(send).toBeEnabled({ timeout: 10000 })
    await send.click()

    // An assistant message bubble appears with non-empty streamed text.
    await expect(
      page.locator('[data-testid="chat-message"][data-role="assistant"]').last(),
    ).toBeVisible({ timeout: 60000 })
    await expect(
      page.locator('[data-testid="chat-message"][data-role="assistant"]').last(),
    ).not.toBeEmpty()
  })
})
