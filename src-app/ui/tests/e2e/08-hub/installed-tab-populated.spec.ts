import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { createAssistantFromHub, getAssistantCards } from './helpers/hub-assistants'

/**
 * E2E — the Hub "Installed" tab POPULATED render + the Re-install action.
 *
 * Audit gap (all-14d6fb1a589c): the only prior InstalledHubTab coverage was
 * the empty-state text (07-hub-version-activation.spec.ts) and the Remove
 * action (installed-tab-actions.spec.ts). This covers two DISTINCT untested
 * aspects of InstalledHubTab.tsx:
 *   1. the populated row metadata — the catalog version Tag (`v{current}`)
 *      that the empty-state test never renders, and
 *   2. the Re-install action (Popconfirm "Re-install" → confirm →
 *      createFromHub replace path → "Re-installed …" success toast),
 *      which installed-tab-actions.spec.ts (Remove only) never drove.
 * Deterministic — no GitHub / real-LLM (uses the seeded hub catalog).
 */

test.describe('Hub — Installed tab populated + Re-install', () => {
  test('a populated row shows its version tag and Re-install replaces it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // --- Install an assistant from the hub so a tracked row exists ---
    await navigateToHub(page, baseURL, 'assistants')
    await waitForHubDataLoad(page)

    const cards = await getAssistantCards(page)
    const firstCard = cards.first()
    const testId = await firstCard.getAttribute('data-testid')
    const hubAssistantId = testId?.replace('hub-assistant-card-', '') ?? ''
    expect(hubAssistantId).toBeTruthy()

    const installedName = `Installed Populated ${Date.now()}`
    await createAssistantFromHub(page, hubAssistantId, { name: installedName })

    // --- Open the Installed tab; the row renders with its metadata ---
    await page.goto(`${baseURL}/hub/installed`)
    await expect(page).toHaveURL(/\/hub\/installed/)

    const rowContainer = page
      .locator('div.flex.items-start', { hasText: installedName })
      .first()
    await expect(rowContainer).toBeVisible({ timeout: 15000 })

    // (1) Populated-render coverage: the version Tag (`v{current}` or
    // `v{installed} → v{current}`) the empty-state-only test never reached.
    await expect(
      rowContainer.locator('.ant-tag').filter({ hasText: /^v/ }).first(),
    ).toBeVisible({ timeout: 10000 })

    // (2) Re-install action: Popconfirm → confirm → replace install.
    const reinstallBtn = rowContainer.getByRole('button', {
      name: /^Re-install$/,
    })
    await expect(reinstallBtn).toBeVisible()
    await expect(reinstallBtn).toBeEnabled()
    await reinstallBtn.click()

    const popconfirm = page.locator('.ant-popconfirm:visible').last()
    await expect(popconfirm).toBeVisible({ timeout: 5000 })
    await popconfirm.getByRole('button', { name: /^Re-install$/ }).click()

    // The reinstall() handler shows a "Re-installed …" success message
    // and reloads the Installed tab; the row remains present afterwards.
    await expect(page.getByText(/Re-installed/).first()).toBeVisible({
      timeout: 15000,
    })
    await expect(
      page.getByText(installedName, { exact: true }).first(),
    ).toBeVisible({ timeout: 15000 })
  })
})
