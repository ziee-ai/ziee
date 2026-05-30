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

  test('shows the unified engine-versions card with platform + backends', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // detect-gpu spawns host probes and can be slow / 502 on a cold backend
    // (the store retries) — give the card time to render.
    const card = page.locator('.ant-card').filter({ hasText: /llamacpp versions/i }).first()
    await expect(card).toBeVisible({ timeout: 30000 })
    await expect(card.getByText(/Platform:/i)).toBeVisible({ timeout: 30000 })
    await expect(card.getByText(/Available backends:/i)).toBeVisible()
  })

  test('shows installed-versions section with empty state', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // No engine downloaded in a fresh test DB → the "Installed versions"
    // section shows an empty state hinting at the section below.
    await expect(page.getByText(/Installed versions/i).first()).toBeVisible()
    await expect(
      page.getByText(/No versions installed yet/i).first(),
    ).toBeVisible()
  })

  test('shows available-versions section (auto-update-check)', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // The update check runs automatically on mount (no "Check for Updates"
    // button). On a backend without network access it'll render
    // "Could not reach the upstream release feed." instead — both are
    // acceptable signals that the section rendered.
    await expect(page.getByText(/Available versions/i).first()).toBeVisible({
      timeout: 30000,
    })
  })

  test('shows the runtime configuration card', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await expect(
      page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).first(),
    ).toBeVisible()
  })
})
