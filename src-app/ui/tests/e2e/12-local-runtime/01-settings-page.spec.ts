import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * Settings → Local Runtimes page surface: the page renders at the
 * correct route (no double-slash bounce), shows both engine tabs, the
 * GPU detection card, the version list, and the runtime config card.
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

  test('shows the GPU detection card', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // detect-gpu spawns host probes and can be slow / 502 on a cold backend
    // (the store retries) — give the card time to render.
    await expect(
      page.locator('.ant-card').filter({ hasText: /Hardware acceleration|GPU/i }).first(),
    ).toBeVisible({ timeout: 30000 })
  })

  test('shows the installed-versions card (empty state)', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // No engine downloaded in a fresh test DB → empty state + a download CTA.
    await expect(page.getByRole('button', { name: /Download Version/i })).toBeVisible()
  })

  test('shows the runtime configuration card', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await expect(
      page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).first(),
    ).toBeVisible()
  })

  test('GPU "Download recommended" opens the download drawer', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const cta = page.getByRole('button', { name: /Download recommended/i })
    if (await cta.isVisible().catch(() => false)) {
      await cta.click()
      await expect(
        page.locator('.ant-drawer-title').filter({ hasText: /Download.*Runtime/i }),
      ).toBeVisible({ timeout: 5000 })
    }
  })
})
