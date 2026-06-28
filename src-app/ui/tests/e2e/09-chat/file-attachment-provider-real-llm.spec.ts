import { mkdtempSync, writeFileSync } from 'fs'
import { tmpdir } from 'os'
import path from 'path'
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
} from './helpers/chat-helpers'
import { attachFileViaUI } from './helpers/file-panel-helpers'

/**
 * E2E — a user file ATTACHED through the chat composer is routed to the provider
 * API and the model can read it (provider_routing → ContentBlock). The existing
 * 09-chat file specs cover the upload advisory / versioning UI, not the
 * attachment → provider round-trip. Real-LLM gated.
 */

const HAS_ANTHROPIC = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — attached file reaches the provider (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM file-attachment E2E skipped')

  test('the model answers from the content of an attached text file', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
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

    // A text file carrying a unique marker the model can only know by reading it.
    const dir = mkdtempSync(path.join(tmpdir(), 'ziee-attach-'))
    const file = path.join(dir, 'secret-note.txt')
    writeFileSync(file, 'The project codename is ZEPHYR_MARKER_8123. Keep it confidential.')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await attachFileViaUI(page, file)
    await sendChatMessage(page, 'Read the attached file and reply with ONLY the project codename it contains.')

    // The assistant's reply reflects the attached file's content.
    await expect(page.locator('body')).toContainText('ZEPHYR_MARKER_8123', { timeout: 90_000 })
  })
})
