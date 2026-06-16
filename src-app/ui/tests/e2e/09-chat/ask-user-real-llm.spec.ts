import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown, sendChatMessage } from './helpers/chat-helpers'

/**
 * ask_user — REAL LLM end-to-end through the actual browser UI.
 *
 * Unlike ask-user-elicitation.spec.ts (which page.route-mocks the SSE stream to
 * test the renderer in isolation), this drives the FULL production path with a
 * REAL Anthropic model and NO mocks: the model decides to call the built-in
 * ask_user tool, the backend emits the form on the real chat stream, the form
 * renders in the UI, the user picks an enum value and submits, the answer POSTs
 * to /elicitation/{id}/respond, and the assistant continues using it.
 *
 * This is the layer that proves the form actually surfaces + the submit click
 * works against the real backend (the class of bug a mocked SSE can't catch).
 * Gated on ANTHROPIC_API_KEY — skips cleanly when unset.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

test.describe('ask_user — real LLM end-to-end (form renders + click works)', () => {
  test.skip(!HAS_ANTHROPIC_KEY, 'ANTHROPIC_API_KEY not set — skipping real-LLM ask_user E2E')
  // Real LLM round-trips (decide-to-ask + continuation) are slow.
  test.slow()

  test('real model calls ask_user → form renders, pick + submit → assistant continues', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(page)
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

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    // Directive prompt forcing a single ask_user call (built-in, auto-attached —
    // no MCP server config needed).
    await sendChatMessage(
      page,
      'I want to pick a color. Use the ask_user tool to ask me to choose exactly one of: ' +
        'red, green, or blue (use an enum schema). Do NOT guess or choose for me — you MUST ' +
        'call ask_user and wait for my answer.',
      false, // don't wait-for-complete: the turn pauses on the form
    )

    // The REAL model calls ask_user → the elicitation form renders in the UI.
    const pending = page.locator('[data-testid^="elicitation-pending-"]').first()
    await expect(pending).toBeVisible({ timeout: 60000 })
    // Attributed to the assistant (not a third-party MCP server).
    await expect(pending).toContainText('Assistant')

    // Pick "green" from the enum Select inside the form. (The field name is
    // model-chosen, so target the Select within the pending form rather than a
    // fixed field testid.) The chat stream reconnect cycles can re-render the
    // message and flicker the antd dropdown, so retry open→pick until the value
    // sticks (Playwright's expect.toPass), then submit.
    const select = pending.locator('.ant-select').first()
    await expect(select).toBeVisible()
    await expect(async () => {
      const current = (await select.textContent())?.toLowerCase() ?? ''
      if (!current.includes('green')) {
        await select.click()
        await page
          .locator('.ant-select-item-option', { hasText: /green/i })
          .first()
          .click({ force: true, timeout: 4000 })
      }
      // Throws (→ retry) until the Select shows the chosen value.
      await expect(select).toContainText('green', { timeout: 2000 })
    }).toPass({ timeout: 30000 })

    // Make sure the option list is closed so it can't overlay the Submit button.
    await page
      .waitForSelector('.ant-select-dropdown', { state: 'hidden', timeout: 3000 })
      .catch(async () => {
        await page.keyboard.press('Escape')
      })
    await pending.locator('[data-testid="elicitation-submit"]').first().click()

    // The form flips to the accepted card — proves the submit click drove a
    // successful /respond POST — and the accepted card shows the chosen value.
    const accepted = page.locator('[data-testid^="elicitation-accepted-"]').first()
    await expect(accepted).toBeVisible({ timeout: 15000 })
    await expect(accepted).toContainText('green')

    // …and the assistant continues, using the answer (real follow-up LLM call).
    await expect
      .poll(
        async () =>
          (await page.locator('[data-role="assistant"]').last().textContent())?.toLowerCase() ?? '',
        { timeout: 60000 },
      )
      .toContain('green')
  })
})

async function getAdminToken(page: import('@playwright/test').Page): Promise<string> {
  const authData = await page.evaluate(() => localStorage.getItem('auth-storage'))
  return JSON.parse(authData!).state.token
}
