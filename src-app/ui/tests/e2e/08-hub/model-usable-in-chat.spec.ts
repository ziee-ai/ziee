import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, getVisibleModelsInDropdown } from '../09-chat/helpers/chat-helpers'

// audit id all-e90df378fce9 — the hub model-download → chat integration was a
// skipped test (a real multi-GB download is impractical in CI). The behavior
// that actually matters downstream — a model, once present, is selectable in the
// chat composer's model picker — is exercised here deterministically with a
// seeded model (the post-download end state), no real download.
test.describe('Model is usable in chat once present', () => {
  test('a registered model appears + is selectable in the chat model picker', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Local', 'local')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    const displayName = `Post-DL Model ${Date.now()}`
    await createModelViaAPI(apiURL, token, providerId, 'post-dl-model', displayName, 'local')

    await goToNewChatPage(page, baseURL)
    const models = await getVisibleModelsInDropdown(page)
    expect(
      models.some(m => m.includes(displayName)),
      `the model must be selectable in chat after registration; saw: ${models.join(', ')}`,
    ).toBeTruthy()
  })
})
