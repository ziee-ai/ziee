import path from 'path'
import type { Page, Locator } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { FILE_ASSETS } from '../chat/helpers/file-panel-helpers'

/**
 * Split-chat E2E — per-pane COMPOSER FILE isolation (TEST-61, ITEM-41).
 *
 * This closes the gap FB-6 reported: "upload a file in one input and see what's
 * going on with the other." No prior spec attached a file into a SPLIT pane's
 * composer and asserted the OTHER pane is unaffected (`right-panel-per-pane`
 * attaches PRE-split; `composer-isolation` covers only model + draft text). It
 * exercises three isolation dimensions the composer must keep per-pane:
 *   1. file attach / remove  — a file attached in pane B appears in B ONLY,
 *      absent in A; removing B's file leaves A's intact.
 *   2. send-enablement       — an in-flight upload in A disables A's Send but
 *      NOT B's (per-pane `useSendBlocker`, keyed by the owning pane).
 *   3. assistant selection   — selecting an assistant in one pane never leaks
 *      its status chip into the other.
 *
 * CRUCIALLY it acts on a pane WITHOUT first focus-clicking that pane's textarea
 * (per FB-4: a focus-click before the assertion masks a shared-focused-bridge
 * bug). The discriminator is always the cross-pane NEGATIVE assertion — under the
 * old shared engine both panes render the focused pane's buffer, so "present in B,
 * absent in A" is exactly what fails when isolation is broken.
 */

/** Split pane 0 = a real conversation, pane 1 = a fresh new-chat pane. */
async function splitIntoTwoPanes(
  page: Page,
  baseURL: string,
  apiURL: string,
  token: string,
): Promise<{ pane0: Locator; pane1: Locator }> {
  const res = await page.request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { title: 'Composer Files A' },
  })
  const convA = (await res.json()).id as string

  await page.goto(`${baseURL}/chat/${convA}`)
  await page.waitForLoadState('load')

  await byTestId(page, 'chat-split-btn').click()
  const pane0 = byTestId(page, 'chat-pane-0')
  const pane1 = byTestId(page, 'chat-pane-1')
  await expect(pane0).toBeVisible({ timeout: 15000 })
  await expect(pane1).toBeVisible({ timeout: 15000 })
  // pane 1 opens the conversation PICKER; "Start a new chat" reaches its own
  // new-chat composer (its own file/assistant ownership key).
  await pane1.getByTestId('pane-start-new-chat').click()
  await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible({ timeout: 15000 })
  return { pane0, pane1 }
}

/**
 * Attach a file into ONE pane's composer. The +-dropdown (and its hidden
 * `input[type=file]`) is portaled to the page root, but it carries the opening
 * pane's React context, so `uploadFiles` runs with THAT pane's ownership key —
 * only ONE dropdown is open at a time, so the single mounted file input is this
 * pane's. We do NOT click the pane's textarea first (no focus-click).
 */
async function attachFileInPane(page: Page, pane: Locator, absPath: string) {
  const filename = path.basename(absPath)
  await pane.getByTestId('chat-input-add-btn').click()
  await page.locator('input[type="file"]').first().setInputFiles(absPath)
  await expect(
    pane.locator(`[data-testid="file-card"][data-filename="${filename}"]`).first(),
  ).toBeVisible({ timeout: 30000 })
}

async function setupProviderAndModel(apiURL: string): Promise<string> {
  const token = await getAdminToken(apiURL)
  const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, token, providerId)
  await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  return token
}

test.describe('Split chat — per-pane composer files', () => {
  test.describe.configure({ retries: 1 })

  test('a file attached in one pane appears in THAT pane only; remove is per-pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await setupProviderAndModel(apiURL)
    const { pane0, pane1 } = await splitIntoTwoPanes(page, baseURL, apiURL, token)

    const mdName = path.basename(FILE_ASSETS.md) // test.md
    const txtName = path.basename(FILE_ASSETS.txt) // test.txt
    const mdCard = (p: Locator) =>
      p.locator(`[data-testid="file-card"][data-filename="${mdName}"]`)
    const txtCard = (p: Locator) =>
      p.locator(`[data-testid="file-card"][data-filename="${txtName}"]`)

    // Attach test.md into pane 1 (the new-chat pane) — WITHOUT focusing it first.
    await attachFileInPane(page, pane1, FILE_ASSETS.md)
    // It is visible in pane 1 and ABSENT from pane 0 (the discriminator: a shared
    // focused-buffer engine would render it in BOTH panes).
    await expect(mdCard(pane1)).toHaveCount(1)
    await expect(mdCard(pane0)).toHaveCount(0)

    // Attach a DIFFERENT file (test.txt) into pane 0. Each pane now holds only its
    // own file — no cross-contamination in either direction.
    await attachFileInPane(page, pane0, FILE_ASSETS.txt)
    await expect(txtCard(pane0)).toHaveCount(1)
    await expect(txtCard(pane1)).toHaveCount(0)
    await expect(mdCard(pane1)).toHaveCount(1)
    await expect(mdCard(pane0)).toHaveCount(0)

    // Remove pane 1's file via its own card's remove control (Trash → confirm);
    // pane 0's file survives untouched (per-pane clearFiles / ownership).
    await mdCard(pane1).hover()
    await mdCard(pane1).getByTestId('file-card-remove-btn').click()
    await byTestId(page, 'file-card-remove-confirm-confirm').click()
    await expect(mdCard(pane1)).toHaveCount(0)
    await expect(txtCard(pane0)).toHaveCount(1)
  })

  test('an in-flight upload disables ONLY its own pane\'s Send button', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await setupProviderAndModel(apiURL)
    const { pane0, pane1 } = await splitIntoTwoPanes(page, baseURL, apiURL, token)

    // Delay the upload RESPONSE (not gate-and-release: that raced route.continue
    // against unroute) so the "uploading" window is observable, then let it
    // complete naturally. The pending uploadingFiles entry is added synchronously
    // on setInputFiles, so the disabled state is observable well within the hold.
    await page.route('**/api/files/upload', async (route) => {
      await new Promise((r) => setTimeout(r, 5000))
      await route.continue()
    })

    const send0 = pane0.getByTestId('chat-input-send-btn')
    const send1 = pane1.getByTestId('chat-input-send-btn')

    // Begin an upload in pane 0 (its uploadingFiles entry is owned by pane 0).
    await pane0.getByTestId('chat-input-add-btn').click()
    await page.locator('input[type="file"]').first().setInputFiles(FILE_ASSETS.md)

    // While pane 0's upload is in flight (within the 5s hold): pane 0's Send is
    // blocked; pane 1's Send is unaffected (per-pane useSendBlocker keyed by the
    // owning pane). This is the discriminator — a shared blocker would disable both.
    await expect(send0).toBeDisabled({ timeout: 4000 })
    await expect(send1).toBeEnabled()

    // After the hold the upload completes on its own → pane 0's card appears and
    // its Send re-enables.
    await expect(
      pane0.locator(`[data-testid="file-card"][data-filename="${path.basename(FILE_ASSETS.md)}"]`),
    ).toBeVisible({ timeout: 30000 })
    await expect(send0).toBeEnabled({ timeout: 15000 })
  })

  test('selecting an assistant in one pane does not leak into the other', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await setupProviderAndModel(apiURL)

    // One assistant to pick.
    const asstResp = await page.request.post(`${apiURL}/api/assistants`, {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        name: 'Pane One Assistant',
        description: 'per-pane assistant isolation',
        instructions: 'You are a per-pane test assistant.',
        is_template: false,
      },
    })
    expect(asstResp.ok()).toBeTruthy()

    const { pane0, pane1 } = await splitIntoTwoPanes(page, baseURL, apiURL, token)

    // Baseline: neither pane shows an assistant status chip.
    await expect(pane0.getByTestId('assistant-status-chip')).toHaveCount(0)
    await expect(pane1.getByTestId('assistant-status-chip')).toHaveCount(0)

    // Open pane 1's + dropdown → assistant submenu → pick the assistant. (The menu
    // portals to the page root but carries pane 1's context, so the selection is
    // keyed to pane 1.)
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'assistant-menu-trigger').click()
    await page.getByText('Pane One Assistant').click()

    // pane 1 now shows the assistant chip; pane 0 shows NONE (per-pane selection
    // key — a shared/focused key would surface the chip in pane 0 too).
    await expect(pane1.getByTestId('assistant-status-chip')).toContainText(
      'Pane One Assistant',
      { timeout: 15000 },
    )
    await expect(pane0.getByTestId('assistant-status-chip')).toHaveCount(0)
  })
})
