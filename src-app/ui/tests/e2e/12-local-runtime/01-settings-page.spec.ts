import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * Settings → Local Runtimes page surface: the page renders at the
 * correct route (no double-slash bounce), shows both engine tabs,
 * the unified engine-versions card (platform + backends + installed
 * + available), and the runtime config card.
 *
 * Engine-free: exercises only the admin UI + read endpoints.
 */
test.describe('Local Runtime — settings page', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('renders at /settings/llm-runtime with engine tabs', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await expect(page.getByRole('tab', { name: 'Llama.cpp' })).toBeVisible()
    await expect(page.getByRole('tab', { name: 'Mistral.rs' })).toBeVisible()
  })

  test('shows the available-versions card with platform + backends', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // detect-gpu spawns host probes and can be slow / 502 on a cold backend
    // (the store retries) — give the card time to render. Platform + Available
    // backends are now inside the Available versions card (they're the
    // precondition for "what's installable for this host").
    const card = page
      .locator('.ant-card')
      .filter({ hasText: /Available versions/i })
      .first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await expect(card.getByText(/Platform:/i)).toBeVisible({ timeout: 30000 })
    await expect(card.getByText(/Available backends:/i)).toBeVisible()
  })

  test('shows installed-versions card with empty state', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // No engine downloaded in a fresh test DB → the dedicated
    // "Installed versions" card shows an empty state hinting at the
    // Available versions card below.
    const card = page
      .locator('.ant-card')
      .filter({ hasText: /Installed versions/i })
      .first()
    await expect(card).toBeVisible()
    await expect(
      card.getByText(/No versions installed yet/i),
    ).toBeVisible()
  })

  test('available-versions card auto-runs the update check (Check-for-updates lives in the card extra)', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // The update check runs automatically on mount. The
    // "Check for updates" button now lives in the Available versions
    // card's `extra` slot for a manual re-run; we just assert the
    // card renders. On a backend without network access the body
    // renders "Could not reach the upstream release feed." instead —
    // both are acceptable signals that the card mounted.
    const card = page
      .locator('.ant-card')
      .filter({ hasText: /Available versions/i })
      .first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await expect(
      card.getByRole('button', { name: /Check for updates/i }),
    ).toBeVisible()
  })

  test('shows the runtime configuration card', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await expect(
      page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).first(),
    ).toBeVisible()
  })
})
