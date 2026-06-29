import { test, expect } from '../../fixtures/test-context'
import {
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — `SummarizationStatusPill` driven through the chat UI.
 *
 * The existing per-conversation-toggle spec drives the summarization-mode
 * endpoints directly; this spec exercises the actual composer **pill**: open a
 * conversation, read the pill's label (default "Summary: auto" = inherit), open
 * its dropdown, pick "Always summarize this conversation", and assert the pill
 * relabels to "Summary: on" and the change persists across a reload.
 *
 * Selectors are semantic: the pill exposes
 * `aria-label="Summarization override: Summary: …"` and the dropdown items use
 * visible menu text.
 */

test.describe('Summarization — composer pill (UI)', () => {
  test('toggle summarization mode via the pill relabels and persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `pill_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit', 'conversations::read', 'conversations::edit'],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const authHeader = { Authorization: `Bearer ${userToken}` }

    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'pill-ui-test' },
    })
    expect(created.ok()).toBe(true)
    const conv = await created.json()

    // Open the conversation — the pill mounts in the composer toolbar.
    await page.goto(`${baseURL}/chat/${conv.id}`)

    // Default mode is `inherit` → label "Summary: auto".
    const pill = byTestId(page, 'summ-mode-tag')
    await expect(pill).toBeVisible({ timeout: 30000 })
    await expect(pill).toHaveAttribute(
      'aria-label',
      /Summary: auto/,
    )

    // Open the dropdown and choose "always summarize".
    await pill.click()
    await byTestId(page, 'summ-mode-dropdown-item-on').click()

    // The pill relabels to "Summary: on".
    await expect(byTestId(page, 'summ-mode-tag')).toHaveAttribute(
      'aria-label',
      /Summary: on/,
      { timeout: 10000 },
    )

    // The mode persists server-side (and is re-read on reload).
    const modeResp = await page.request.get(
      `${apiURL}/api/conversations/${conv.id}/summarization-mode`,
      { headers: authHeader },
    )
    expect(modeResp.ok()).toBe(true)
    expect(((await modeResp.json()) as { summarization_mode: string }).summarization_mode).toBe('on')

    await page.reload()
    await expect(byTestId(page, 'summ-mode-tag')).toHaveAttribute(
      'aria-label',
      /Summary: on/,
      { timeout: 30000 },
    )
  })
})
