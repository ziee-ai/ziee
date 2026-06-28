import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

// audit id all-aed242b6d0f2 — the chat composer "+" (Add attachment) dropdown
// aggregates slot-contributed menu items (file attach, MCP tools, skills,
// assistant). Its rendering was untested. This opens the dropdown and asserts
// the file + MCP items render. (The skills item is conversation-gated and is
// covered by 16-skills/chat-skill-opt-out.spec.ts.)
test.describe('Chat composer — "+" dropdown', () => {
  test('opens and shows the file + MCP menu items', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    // A local provider+model needs no API key but makes the composer usable.
    const providerId = await createProviderViaAPI(apiURL, token, 'Local', 'local')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'local')

    await goToNewChatPage(page, baseURL)

    await page.getByRole('button', { name: 'Add attachment' }).click()

    await expect(page.getByText('Attach files or photos')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('MCP tools & servers')).toBeVisible()
  })
})
