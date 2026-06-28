import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createModelViaAPI } from '../../common/provider-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

/**
 * E2E (deterministic, no LLM) — the empty-key validation branch of
 * ProviderApiKeyModal.
 *
 * Audit gap (all-cd023637008d): model-key-modal-send-journey.spec covers the
 * happy path (fill a real key → save → send), but the EMPTY-key guard
 * (ProviderApiKeyModal.tsx:31-34 — `if (!trimmed) setError('API key cannot
 * be empty')`) was never exercised. This selects a keyless model to open the
 * modal, clicks "Save & Select Model" with a blank field, and asserts the
 * inline error renders, the modal stays open, and NO save-key request fires.
 */

async function assignToAdminGroup(
  apiURL: string,
  auth: Record<string, string>,
  providerId: string,
) {
  const groups = await (
    await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })
  ).json()
  const arr = Array.isArray(groups) ? groups : groups.groups || []
  const admin = arr.find((g: { name: string }) => g.name === 'Administrators')
  await fetch(`${apiURL}/api/groups/${admin.id}/providers`, {
    method: 'PUT',
    headers: auth,
    body: JSON.stringify({ provider_ids: [providerId] }),
  })
}

test.describe('LLM — model key modal empty-key validation', () => {
  test('submitting an empty API key is blocked with an inline error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    }

    // A KEYED openai model so the picker bootstraps without a modal.
    const keyedId = (
      await (
        await fetch(`${apiURL}/api/llm-providers`, {
          method: 'POST',
          headers: auth,
          body: JSON.stringify({
            name: 'AAA Keyed',
            provider_type: 'openai',
            enabled: true,
            api_key: 'sk-keyed',
          }),
        })
      ).json()
    ).id
    await createModelViaAPI(apiURL, token, keyedId, 'keyed-model', 'Keyed Model', 'openai')
    await assignToAdminGroup(apiURL, auth, keyedId)

    // A KEYLESS anthropic provider + Haiku model → selecting it opens the modal.
    const anthId = (
      await (
        await fetch(`${apiURL}/api/llm-providers`, {
          method: 'POST',
          headers: auth,
          body: JSON.stringify({
            name: 'ZZZ Anthropic Keyless',
            provider_type: 'anthropic',
            enabled: true,
          }),
        })
      ).json()
    ).id
    await createModelViaAPI(
      apiURL,
      token,
      anthId,
      'claude-haiku-4-5-20251001',
      'Haiku Keyless',
      'anthropic',
    )
    await assignToAdminGroup(apiURL, auth, anthId)

    // Track any attempt to persist a user API key — there must be none.
    let saveKeyAttempts = 0
    page.on('request', req => {
      if (
        req.method() === 'POST' &&
        req.url().includes('/api/user-llm-providers/api-keys')
      ) {
        saveKeyAttempts++
      }
    })

    await goToNewChatPage(page, baseURL)

    // Select the keyless Haiku model → the API-key modal appears.
    await page.click('[data-testid="model-selector"] .ant-select')
    await page.getByRole('option', { name: 'Haiku Keyless' }).first().click()
    const modal = page.getByRole('dialog').filter({ hasText: 'API Key Required' })
    await expect(modal).toBeVisible({ timeout: 10000 })

    // Leave the key field empty and click "Save & Select Model".
    await modal.getByRole('button', { name: 'Save & Select Model' }).click()

    // The inline validation error renders and the modal stays open.
    await expect(modal.getByText('API key cannot be empty')).toBeVisible({
      timeout: 5000,
    })
    await expect(modal).toBeVisible()

    // No save-key request was ever issued for the empty submission.
    expect(saveKeyAttempts).toBe(0)
  })
})
