import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createProviderViaAPI, createModelViaAPI } from '../../common/provider-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

/**
 * E2E for the chat model selector's API-key gating.
 *
 * - A REMOTE provider with no key configured (here a keyless `custom` provider)
 *   prompts the user to enter their own key inline when one of its models is
 *   selected.
 * - A LOCAL provider NEVER prompts: it authenticates via an internal,
 *   server-minted proxy token, so selecting a local model selects it directly
 *   with no modal. (This used to be tested with a `local` provider as the
 *   "keyless" case — that was the bug; local must NOT be gated.)
 */

test.describe('Provider API key modal (chat model selector)', () => {
  // A provider is visible to a user only if enabled AND assigned to a group the
  // user belongs to. The setup admin is in "Administrators"; assign there.
  async function assignToAdminGroup(
    apiURL: string,
    auth: Record<string, string>,
    providerId: string,
  ) {
    const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })
    const { groups } = await groupsRes.json()
    const adminGroupId = groups.find((g: any) => g.name === 'Administrators').id
    const r = await fetch(`${apiURL}/api/llm-providers/${providerId}/groups`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ group_id: adminGroupId }),
    })
    if (!r.ok) throw new Error(`assign ${providerId} failed: ${r.status} ${await r.text()}`)
  }

  test('selecting a keyless remote-provider model prompts for an API key', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // Keyed provider + model (api_key_configured = true → no modal). Inlined
    // with a literal key so it doesn't depend on OPENAI_API_KEY being set.
    const keyedRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: 'Keyed Provider', provider_type: 'openai', enabled: true, api_key: 'sk-keyed' }),
    })
    if (!keyedRes.ok) throw new Error(`keyed provider create failed: ${keyedRes.status}`)
    const keyedId = (await keyedRes.json()).id
    await createModelViaAPI(apiURL, token, keyedId, 'keyed-model', 'Keyed Model', 'openai')
    await assignToAdminGroup(apiURL, auth, keyedId)

    // Keyless CUSTOM provider + model (no key → still triggers the modal, since
    // a remote/custom endpoint legitimately needs a key). `custom` is accepted
    // enabled-without-key by the backend; `createProviderViaAPI`'s type union
    // doesn't include it, so create it inline.
    const customRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: 'Keyless Custom', provider_type: 'custom', enabled: true }),
    })
    if (!customRes.ok) throw new Error(`custom provider create failed: ${customRes.status} ${await customRes.text()}`)
    const customId = (await customRes.json()).id
    await createModelViaAPI(apiURL, token, customId, 'custom-model', 'Custom Model')
    await assignToAdminGroup(apiURL, auth, customId)

    await goToNewChatPage(page, baseURL)

    // "Keyed Provider" sorts before "Keyless Custom", so the keyed model is the
    // default selection; switching to the custom model fires onChange.
    await expect(page.locator('[data-testid="model-selector"]')).toContainText('Keyed Model')
    await page.click('[data-testid="model-selector"] .ant-select')
    const customOption = page.getByRole('option', { name: 'Custom Model' })
    await customOption.waitFor({ state: 'visible' })
    await customOption.click()

    // The modal appears because the custom provider has no key configured.
    const modal = page.getByRole('dialog').filter({ hasText: 'API Key Required' })
    await expect(modal).toBeVisible({ timeout: 10000 })
    await expect(modal.getByText(/API Key Required — Keyless Custom/)).toBeVisible()

    // Enter a key and save → modal closes.
    await modal.locator('input[type="password"]').fill('sk-my-key')
    await modal.getByRole('button', { name: 'Save & Select Model' }).click()
    await expect(modal).toBeHidden({ timeout: 10000 })
  })

  test('selecting a local-provider model does NOT prompt for an API key', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // Keyed provider + model — gives a non-local starting selection so switching
    // to the local model fires onChange.
    const keyedRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: 'Keyed Provider', provider_type: 'openai', enabled: true, api_key: 'sk-keyed' }),
    })
    if (!keyedRes.ok) throw new Error(`keyed provider create failed: ${keyedRes.status}`)
    const keyedId = (await keyedRes.json()).id
    await createModelViaAPI(apiURL, token, keyedId, 'keyed-model', 'Keyed Model', 'openai')
    await assignToAdminGroup(apiURL, auth, keyedId)

    // Local provider + model. Local providers never need a user API key.
    const localId = await createProviderViaAPI(apiURL, token, 'Local Provider', 'local')
    await createModelViaAPI(apiURL, token, localId, 'local-model', 'Local Model', 'local')
    await assignToAdminGroup(apiURL, auth, localId)

    await goToNewChatPage(page, baseURL)

    // Providers sort Local-first, so the local model is the default
    // selection. To prove that *switching to* a local model fires onChange
    // without prompting, first move OFF local onto the keyed model (its
    // provider is configured with a key, so no modal), then switch back.
    await expect(page.locator('[data-testid="model-selector"]')).toContainText('Local Model')
    await page.click('[data-testid="model-selector"] .ant-select')
    const keyedOption = page.getByRole('option', { name: 'Keyed Model' })
    await keyedOption.waitFor({ state: 'visible' })
    await keyedOption.click()
    await expect(page.locator('[data-testid="model-selector"]')).toContainText('Keyed Model')

    await page.click('[data-testid="model-selector"] .ant-select')
    const localOption = page.getByRole('option', { name: 'Local Model' })
    await localOption.waitFor({ state: 'visible' })
    await localOption.click()

    // Prove the selection settled first (this retries until it passes), so any
    // erroneously-mounted modal has had time to appear...
    await expect(page.locator('[data-testid="model-selector"]')).toContainText('Local Model')
    // ...THEN assert the modal never appeared — local providers select directly.
    const modal = page.getByRole('dialog').filter({ hasText: 'API Key Required' })
    await expect(modal).toHaveCount(0)
  })
})
