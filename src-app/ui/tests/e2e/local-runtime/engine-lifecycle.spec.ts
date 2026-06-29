import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { assignProviderToAdministratorsGroup } from '../../common/provider-helpers'
import { byTestId } from '../testid.ts'
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
    // The fork releases aren't cosign-signed, but the runtime no longer
    // gates downloads on a signed-only policy — the install proceeds
    // unconditionally (cosign verify is logged but doesn't block).
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // AvailableVersionsCard auto-checks for updates on mount; wait for the
    // "Available versions" card to populate, then click the first available
    // (non-installed) Install button (derived id `llmrt-version-install-<ver>`).
    await expect(byTestId(page, 'llmrt-available-versions-card')).toBeVisible({ timeout: 30000 })
    const firstAvailable = page.locator('[data-testid^="llmrt-version-install-"]').first()
    await expect(firstAvailable).toBeVisible({ timeout: 30000 })
    await firstAvailable.click()

    // The downloaded version row appears in the installed-versions list (it
    // becomes the default + a Delete action shows for it).
    await expect(
      page.locator('[data-testid^="llmrt-version-delete-"]').first()
    ).toBeVisible({ timeout: 120000 })
  })

  test('chat auto-starts a stopped engine and streams a reply', async ({ page, testInfra }) => {
    // Cold-CPU first-token after engine spawn is slow on commodity Macs.
    // The chain is: GGUF download (~30s with cache) → engine binary
    // spawn → llama-server cold model load (~3–5 min on Apple Silicon
    // CPU for TinyLlama Q4_K_M) → MCP tool description plumbing →
    // prompt eval + first-token (~1–2 min on CPU). 20-min budget
    // covers worst-case slow CPU.
    test.setTimeout(1200000)
    const { baseURL, apiURL } = testInfra
    const token = await getCurrentUserToken(page)

    // Real engine + real GGUF model under a local provider, exposed
    // to chat.
    await downloadEngineViaApi(baseURL, token, 'llamacpp')
    const providerId = await seedLocalProvider(baseURL, token)
    await downloadGgufModelViaApi(baseURL, token, providerId)
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)

    // Fresh load so the chat model store picks up the new local model (the only
    // model → auto-selected in the picker).
    await page.goto(`${baseURL}/`)
    await page.waitForLoadState('load')

    const textarea = byTestId(page, 'chat-message-textarea')
    await expect(textarea).toBeVisible()
    await textarea.fill('Reply with the single word: ok')
    await textarea.press('Enter')

    // The user message renders immediately…
    await expect(
      byTestId(page, 'chat-message').filter({ hasText: 'Reply with the single word' })
    ).toBeVisible({ timeout: 15000 })

    // …then the engine auto-starts and streams the assistant reply.
    await expect(byTestId(page, 'chat-message').nth(1)).toBeVisible({
      timeout: 900000
    })
  })
})
