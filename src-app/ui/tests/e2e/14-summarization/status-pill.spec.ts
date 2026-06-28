import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the per-conversation SummarizationStatusPill (composer toolbar_status,
 * SummarizationStatusPill.tsx). per-conversation-toggle.spec drives the
 * summarization-mode API directly; this drives the actual PILL UI. The pill
 * only renders when (a) there's an active conversation and (b) summarization
 * isn't globally disabled by the admin.
 */
test.describe('Summarization — composer status pill', () => {
  test('dropdown switches the per-conversation summarization mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    // Enable summarization deployment-wide so the pill is allowed to render.
    const enable = await page.request.put(
      `${apiURL}/api/summarization/settings`,
      { headers: auth, data: { enabled: true } },
    )
    expect(enable.ok()).toBe(true)

    // A real conversation to bind the pill to.
    const conv = await page.request.post(`${apiURL}/api/conversations`, {
      headers: auth,
      data: { title: 'summary-pill-test' },
    })
    const conversationId: string = (await conv.json()).id

    await page.goto(`${baseURL}/chat/${conversationId}`)

    // Defaults to 'auto' (inherit).
    const pill = page.getByRole('button', { name: /Summarization override:/ })
    await expect(pill).toBeVisible({ timeout: 30000 })
    await expect(pill).toHaveText(/Summary: auto/)

    // Open the dropdown and pick "Always summarize this conversation" → on.
    await pill.click()
    await page
      .getByRole('menuitem', { name: 'Always summarize this conversation' })
      .click()

    await expect(
      page.getByText('Summarization: on for this conversation'),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Summarization override: Summary: on' }),
    ).toBeVisible()

    // Server persisted the choice.
    const after = await page.request.get(
      `${apiURL}/api/conversations/${conversationId}/summarization-mode`,
      { headers: auth },
    )
    expect((await after.json()).summarization_mode).toBe('on')
  })
})
