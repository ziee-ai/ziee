import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { createAssistantFromHub, getAssistantCards } from './helpers/hub-assistants'

/**
 * E2E — the Hub "Installed" tab POPULATED render + the Re-install action.
 *
 * Audit gap (all-14d6fb1a589c): the only prior InstalledHubTab coverage was
 * the empty-state text (hub-version-activation.spec.ts) and the Remove
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

    // The one-click "Use Assistant" hub flow installs with the manifest name
    // (there is no name-customization UI), so we identify the row by position:
    // a fresh per-test DB has exactly one hub-installed entity (this one).
    await createAssistantFromHub(page, hubAssistantId)

    // --- Open the Installed tab; the row renders with its metadata ---
    await page.goto(`${baseURL}/hub/installed`)
    await expect(page).toHaveURL(/\/hub\/installed/)

    const rowContainer = page.getByTestId(/^hub-installed-row-/).first()
    await expect(rowContainer).toBeVisible({ timeout: 15000 })

    // (1) Populated-render coverage: the version Tag (`v{current}` or
    // `v{installed} → v{current}`) the empty-state-only test never reached.
    await expect(
      rowContainer.getByTestId(/^hub-installed-version-tag-/).first(),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      rowContainer.getByTestId(/^hub-installed-version-tag-/).first(),
    ).toContainText(/^v/)

    // (2) Re-install action: Confirm dialog → confirm → replace install.
    const reinstallBtn = rowContainer.getByTestId(/^hub-installed-reinstall-btn-/)
    await expect(reinstallBtn).toBeVisible()
    await expect(reinstallBtn).toBeEnabled()
    await reinstallBtn.click()

    // Target the Confirm OK button specifically — the prefix regex also
    // matches the dialog content + the cancel button (3 nodes → strict-mode
    // violation).
    const confirm = page.locator(
      '[data-testid^="hub-installed-reinstall-confirm-"][data-testid$="-confirm"]',
    )
    await expect(confirm).toBeVisible({ timeout: 5000 })
    await confirm.click()

    // The reinstall() handler shows a "Re-installed …" success toast and
    // reloads the Installed tab; the row remains present afterwards.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 15000 })
    await expect(
      page.getByTestId(/^hub-installed-row-/).first(),
    ).toBeVisible({ timeout: 15000 })
  })
})
