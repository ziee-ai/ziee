import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the 3-way open-conversation choice prompt (TEST-63, ITEM-43 /
 * FB-8 / DEC-58). A plain sidebar click on a conversation NOT already in a pane,
 * while a split is open, asks the user how to place it: Open as single pane /
 * Replace the active pane / Add as a new pane. It does NOT prompt in single-pane
 * mode, nor when the clicked conversation is already in a pane (it focuses that
 * pane). No LLM.
 */
test.describe('Split chat — open-conversation choice prompt', () => {
  test.describe.configure({ retries: 1 })

  const mkConv = async (
    page: Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  /** Build a `[A | B]` split (pane 0 focused) and return the three conversation ids. */
  async function splitAB(page: Page, baseURL: string, apiURL: string, token: string) {
    const convA = await mkConv(page, apiURL, token, 'Choice Alpha')
    const convB = await mkConv(page, apiURL, token, 'Choice Bravo')
    const convC = await mkConv(page, apiURL, token, 'Choice Charlie')
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 20000 })
    await rowB.hover()
    await byTestId(page, `chat-recent-row-actions-btn-${convB}`).click()
    await byTestId(page, `chat-recent-row-menu-${convB}-item-open-in-split`).click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-0').click() // focus pane 0 (conv A)
    await expect(byTestId(page, 'chat-pane-0')).toHaveClass(/ring-primary/)
    return { convA, convB, convC }
  }

  test('"Add as a new pane" opens a third pane', async ({ page, testInfra }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { convC } = await splitAB(page, baseURL, apiURL, token)

    await byTestId(page, `chat-recent-conversations-menu-item-${convC}`).click()
    await expect(byTestId(page, 'open-conversation-choice')).toBeVisible()
    await byTestId(page, 'open-conversation-choice-opt-new').click()
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
  })

  test('"Replace the active pane" retargets the focused pane and keeps the split', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { convC } = await splitAB(page, baseURL, apiURL, token)

    await byTestId(page, `chat-recent-conversations-menu-item-${convC}`).click()
    await expect(byTestId(page, 'open-conversation-choice')).toBeVisible()
    await byTestId(page, 'open-conversation-choice-opt-replace').click()
    // Still exactly two panes; the focused pane now shows Charlie.
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)
    await expect(
      byTestId(page, 'chat-pane-0').getByTestId('conversation-title'),
    ).toContainText('Charlie')
  })

  test('"Open as single pane" collapses the split to a single-pane view', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { convC } = await splitAB(page, baseURL, apiURL, token)

    await byTestId(page, `chat-recent-conversations-menu-item-${convC}`).click()
    await expect(byTestId(page, 'open-conversation-choice')).toBeVisible()
    await byTestId(page, 'open-conversation-choice-opt-single').click()
    // The split is gone (no columns / panes); a single-pane view of Charlie remains.
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)
    await expect(byTestId(page, 'conversation-title')).toContainText('Charlie')
  })

  test('no prompt in single-pane mode — a plain click just navigates', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Solo Alpha')
    const convB = await mkConv(page, apiURL, token, 'Solo Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 20000 })
    await rowB.click()
    // No prompt — it navigated straight to B.
    await expect(byTestId(page, 'open-conversation-choice')).toHaveCount(0)
    await expect(page).toHaveURL(new RegExp(convB))
  })

  test('no prompt when clicking a conversation already in a pane — it focuses that pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { convB } = await splitAB(page, baseURL, apiURL, token)

    // Pane 0 is focused; plain-click B (already in pane 1) → focus pane 1, NO prompt.
    await byTestId(page, `chat-recent-conversations-menu-item-${convB}`).click()
    await expect(byTestId(page, 'open-conversation-choice')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-1')).toHaveClass(/ring-primary/)
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)
  })
})
