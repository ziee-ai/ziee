import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { createAssistantFromHub, getAssistantCards } from './helpers/hub-assistants'

/**
 * E2E — the Hub "Installed" tab per-row actions.
 *
 * Audit gap (all-a568ff5f3043): existing hub specs only assert the
 * EMPTY Installed tab. InstalledHubTab.tsx renders Re-install / Remove
 * actions per installed row (each behind an AntD Popconfirm), but no
 * spec drove either action. This installs an assistant from the hub
 * (so a tracked row exists), then exercises the real Remove action:
 * Popconfirm → confirm → DELETE /api/assistants/{id} → the row clears
 * from the Assistants card. Deterministic (no GitHub / real-LLM).
 */

test.describe('Hub — Installed tab actions', () => {
  test('Remove deletes an installed assistant row', async ({
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

    const installedName = `Installed E2E ${Date.now()}`
    await createAssistantFromHub(page, hubAssistantId, { name: installedName })

    // --- Go to the Installed tab; the new install shows in Assistants ---
    await page.goto(`${baseURL}/hub/installed`)
    await expect(page).toHaveURL(/\/hub\/installed/)

    // Locate the tracked row by the name this test created (dynamic data,
    // so a hasText filter is allowed). The Remove button lives in the row.
    const rowContainer = page
      .getByTestId(/^hub-installed-row-/)
      .filter({ hasText: installedName })
      .first()
    await expect(rowContainer).toBeVisible({ timeout: 15000 })

    const removeBtn = rowContainer.getByTestId(/^hub-installed-remove-btn-/)
    await expect(removeBtn).toBeVisible()

    // --- Drive the real Remove action: Confirm dialog → confirm ---
    await removeBtn.click()
    const confirm = page.getByTestId(/^hub-installed-remove-confirm-/)
    await expect(confirm).toBeVisible({ timeout: 5000 })

    const deleteResp = page.waitForResponse(
      r =>
        /\/api\/assistants\//.test(r.url()) && r.request().method() === 'DELETE',
      { timeout: 15000 },
    )
    // The confirm OK button derives `${confirmTestid}-confirm`.
    await page
      .locator(
        '[data-testid^="hub-installed-remove-confirm-"][data-testid$="-confirm"]',
      )
      .click()

    const resp = await deleteResp
    expect(resp.status()).toBeLessThan(300)

    // Success toast + the row clears from the Installed tab.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 10000 })
    await expect(
      page.getByTestId(/^hub-installed-row-/).filter({ hasText: installedName }),
    ).toHaveCount(0, { timeout: 15000 })
  })
})
