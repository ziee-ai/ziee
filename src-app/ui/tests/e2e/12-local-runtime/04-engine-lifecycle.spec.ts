import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { assignProviderToAdministratorsGroup } from '../../common/provider-helpers'
import {
  gotoRuntimeSettings,
  downloadEngineViaApi,
  downloadGgufModelViaApi,
  seedLocalProvider
} from './helpers/local-runtime-helpers'

/**
 * Engine-dependent flows against the REAL `ziee-ai/*` GitHub releases and a
 * REAL TinyLlama GGUF from HuggingFace — the path `gold_smoke` proves on the
 * backend, now driven through the UI. Gated on `HUGGINGFACE_API_KEY` (source
 * `server/tests/.env.test` first; the backend inherits it from the shell env).
 *
 * NOT YET RUN — real network + ~670 MB download + CPU inference. The chat
 * selectors + long inference timeouts will need a verification pass.
 */
const HF_KEY = process.env.HUGGINGFACE_API_KEY

test.describe('Local Runtime — engine lifecycle (needs HUGGINGFACE_API_KEY)', () => {
  test.skip(!HF_KEY, 'set HUGGINGFACE_API_KEY (source server/tests/.env.test) to run real-engine flows')

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('download an engine version via the inline Available versions list', async ({ page, testInfra }) => {
    // The fork releases aren't cosign-signed → the download path refuses unless
    // allow_unsigned_downloads is on. Enable it, then drive the UI download.
    const token = await getCurrentUserToken(page)
    await fetch(`${testInfra.baseURL}/api/local-runtime/settings`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ allow_unsigned_downloads: true })
    })

    await gotoRuntimeSettings(page, testInfra.baseURL)
    // EngineVersionsCard auto-checks for updates on mount; wait for the
    // "Available versions" section to populate, then click the Download
    // button on the first available (non-installed) row.
    const pane = page.locator('.ant-tabs-tabpane-active')
    await expect(pane.getByText(/Available versions/i).first()).toBeVisible({ timeout: 30000 })
    const firstAvailable = pane.getByRole('button', { name: /^Download$/ }).first()
    await expect(firstAvailable).toBeVisible({ timeout: 30000 })
    await firstAvailable.click()

    // The downloaded version row appears in the installed-versions list (it
    // becomes the default + a Delete action shows for it).
    await expect(
      pane.getByRole('button', { name: 'Delete' }).first()
    ).toBeVisible({ timeout: 120000 })
  })

  test('chat auto-starts a stopped engine and streams a reply', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getCurrentUserToken(page)

    // Real engine + real GGUF model under a local provider, exposed to chat.
    await downloadEngineViaApi(baseURL, token, 'llamacpp')
    const providerId = await seedLocalProvider(baseURL, token)
    await downloadGgufModelViaApi(baseURL, token, providerId)
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)

    // Fresh load so the chat model store picks up the new local model (the only
    // model → auto-selected in the picker).
    await page.goto(`${baseURL}/`)
    await page.waitForLoadState('networkidle')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await expect(textarea).toBeVisible()
    await textarea.fill('Reply with the single word: ok')
    await textarea.press('Enter')

    // The user message renders immediately…
    await expect(
      page
        .locator('[data-testid="chat-message"]')
        .filter({ hasText: 'Reply with the single word' })
    ).toBeVisible({ timeout: 15000 })

    // …then the engine auto-starts and streams the assistant reply. CPU
    // first-token after a cold start is slow — allow several minutes.
    await expect(page.locator('[data-testid="chat-message"]').nth(1)).toBeVisible({
      timeout: 240000
    })
  })
})
