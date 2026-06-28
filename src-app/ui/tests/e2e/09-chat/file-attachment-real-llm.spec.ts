// Run with --workers=1 (shared backend + test DB).
import path from 'path'
import { fileURLToPath } from 'url'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from './helpers/chat-helpers'
import { attachFileViaUI } from './helpers/file-panel-helpers'

// audit id all-3e6d5a51bd95 — no E2E exercised a file attached THROUGH THE CHAT
// UI actually reaching the provider (the existing file specs cover the upload
// advisory + versioning UI only, never the provider round-trip). This attaches a
// real text file via the composer, sends it to a real Anthropic model, and
// asserts the model's reply contains a secret string that exists ONLY inside the
// attachment — proving provider_routing inlined the file content to the model.
// Gated on ANTHROPIC_API_KEY (the established real-LLM E2E pattern).

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const MARKER_FIXTURE = path.resolve(__dirname, 'fixtures/attachment-marker.txt')
const SECRET_CODE = 'ZIEE_ATTACH_7788'

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('Chat — file attachment reaches the provider (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping real-LLM attachment E2E')
  test.slow()

  test('model answers using content found only in the attached file', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    // Attach the real text file through the composer "+" flow.
    await attachFileViaUI(page, MARKER_FIXTURE)

    // Ask for the secret that exists ONLY in the attachment.
    await sendChatMessage(
      page,
      'Read the attached file and reply with ONLY the secret project code it contains.',
      false,
    )

    // The assistant's reply must echo the code → the attachment reached the model.
    await expect
      .poll(
        async () => (await page.locator('[data-role="assistant"]').last().textContent()) ?? '',
        { timeout: 90000 },
      )
      .toContain(SECRET_CODE)
  })
})
