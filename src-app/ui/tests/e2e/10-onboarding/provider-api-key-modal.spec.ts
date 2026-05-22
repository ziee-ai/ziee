import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createProviderViaAPI, createModelViaAPI } from '../../common/provider-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

/**
 * E2E for ProviderApiKeyModal: selecting a model whose provider has no API key
 * configured prompts the user to enter their own key inline in the model
 * selector. A `local` provider is created without a key (api_key_configured =
 * false); a second keyed provider gives us a non-keyless model to start from,
 * so switching to the keyless model fires the change that opens the modal.
 */

test.describe('Provider API key modal (chat model selector)', () => {
  test('selecting a keyless-provider model prompts for an API key', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // Resolve the Administrators group (the admin is a member).
    const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })
    const { groups } = await groupsRes.json()
    const adminGroupId = groups.find((g: any) => g.name === 'Administrators').id

    // Additive assign (per-provider endpoint; the group-centric one replaces).
    const assign = async (providerId: string) => {
      const r = await fetch(`${apiURL}/api/llm-providers/${providerId}/groups`, {
        method: 'POST',
        headers: auth,
        body: JSON.stringify({ group_id: adminGroupId }),
      })
      if (!r.ok) throw new Error(`assign ${providerId} failed: ${r.status} ${await r.text()}`)
    }

    // Keyed provider + model (api_key_configured = true → no modal).
    const keyedRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: 'Keyed Provider', provider_type: 'openai', enabled: true, api_key: 'sk-keyed' }),
    })
    if (!keyedRes.ok) throw new Error(`keyed provider create failed: ${keyedRes.status}`)
    const keyedId = (await keyedRes.json()).id
    await createModelViaAPI(apiURL, token, keyedId, 'keyed-model', 'Keyed Model', 'openai')
    await assign(keyedId)

    // Keyless local provider + model (no key → triggers the modal).
    const keylessId = await createProviderViaAPI(apiURL, token, 'Keyless Provider', 'local')
    await createModelViaAPI(apiURL, token, keylessId, 'keyless-model', 'Keyless Model', 'local')
    await assign(keylessId)

    await goToNewChatPage(page, baseURL)

    // "Keyed Provider" sorts before "Keyless Provider", so the keyed model is
    // the default selection; switching to the keyless model fires onChange.
    await expect(page.locator('[data-testid="model-selector"]')).toContainText('Keyed Model')
    await page.click('[data-testid="model-selector"] .ant-select')
    const keylessOption = page.getByRole('option', { name: 'Keyless Model' })
    await keylessOption.waitFor({ state: 'visible' })
    await keylessOption.click()

    // The modal appears because the keyless provider has no key configured.
    const modal = page.getByRole('dialog').filter({ hasText: 'API Key Required' })
    await expect(modal).toBeVisible({ timeout: 10000 })
    await expect(modal.getByText(/API Key Required — Keyless Provider/)).toBeVisible()

    // Enter a key and save → modal closes.
    await modal.locator('input[type="password"]').fill('sk-my-key')
    await modal.getByRole('button', { name: 'Save & Select Model' }).click()
    await expect(modal).toBeHidden({ timeout: 10000 })
  })
})
