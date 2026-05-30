import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'
import {
  gotoRuntimeSettings,
  seedLocalProvider,
  seedLocalModel,
  downloadEngineViaApi,
  downloadGgufModelViaApi
} from './helpers/local-runtime-helpers'

/**
 * Frontend permission gating for the Local Runtime settings page. Verifies the
 * UI mirrors the backend's per-endpoint `RequirePermissions` model — i.e. a
 * principal only sees the affordances for permissions it actually holds. The
 * backend enforces every endpoint regardless, so these are UX/consistency
 * guarantees, not the security boundary.
 *
 * Covered:
 *  - `versions_read` gates the version-catalogue / update-checker / per-version
 *    usage sections (the page route only requires `read`).
 *  - `manage` gates the model start/stop/restart/swap controls.
 *  - `logs` gates the Logs control INDEPENDENTLY of `manage` (a logs-only user
 *    sees Logs but not Stop; a manage-only user sees Stop but not Logs).
 */
const HF_KEY = process.env.HUGGINGFACE_API_KEY

// Baseline reads so the settings page renders without an unrelated card (the
// runtime-config card reads `settings_read`) erroring; `versions_read` is then
// the only variable for the version sections.
const BASE_READS = [Permissions.LocalRuntimeRead, Permissions.RuntimeSettingsRead]

// The Installed versions card is the row-level surface for
// model start/stop/swap controls now (the standalone "Models by
// engine version" card was folded in). Engine-backed permission
// tests scope assertions to this card.
function installedCard(page: Page) {
  return page
    .locator('.ant-tabs-tabpane-active')
    .locator('.ant-card')
    .filter({ hasText: 'Installed versions' })
}

// ── engine-free: versions_read gates whole sections ──────────────────────
test.describe('Local Runtime — permission gating (engine-free)', () => {
  test('versions_read gates the installed-versions + available-versions cards', async ({
    page,
    testInfra
  }) => {
    const { baseURL, apiURL } = testInfra

    // read (no versions_read): page loads, but both per-engine
    // cards are hidden — they're gated together on versions_read
    // (detect-gpu + check-updates + version list all need it).
    await loginWithPerms(page, baseURL, apiURL, BASE_READS, 'lrt-noversions')
    await gotoRuntimeSettings(page, baseURL)
    await expect(page.getByRole('tab', { name: 'Llama.cpp' })).toBeVisible()
    await expect(page.getByText(/Available backends:/i)).toHaveCount(0)
    await expect(page.getByText(/Available versions/i)).toHaveCount(0)
    await expect(page.getByText(/Installed versions/i)).toHaveCount(0)

    // + versions_read: both cards appear, and the Available versions
    // card surfaces a manual "Check for updates" button in its
    // `extra` slot.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead],
      'lrt-versions'
    )
    await gotoRuntimeSettings(page, baseURL)
    await expect(page.getByText(/Available backends:/i).first()).toBeVisible({
      timeout: 30000,
    })
    await expect(page.getByText(/Available versions/i).first()).toBeVisible()
    await expect(page.getByText(/Installed versions/i).first()).toBeVisible()
    await expect(
      page.getByRole('button', { name: /Check for updates/i }),
    ).toBeVisible()
  })
})

// ── engine-backed: manage gates Start; logs gates Logs independently ──────
test.describe('Local Runtime — permission gating (needs HUGGINGFACE_API_KEY)', () => {
  test.skip(!HF_KEY, 'set HUGGINGFACE_API_KEY (source server/tests/.env.test) for engine-backed gating')

  test('manage gates the model Start control', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // Admin seeds an engine version + a non-running model that resolves to it.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getCurrentUserToken(page)
    await downloadEngineViaApi(baseURL, adminToken, 'llamacpp', 'v0.0.1-alpha', true)
    const providerId = await seedLocalProvider(baseURL, adminToken)
    await seedLocalModel(baseURL, adminToken, providerId, `e2e-gate-${Date.now()}`)

    // read + versions_read, NO manage: model row visible, no Start button.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead],
      'lrt-nomanage'
    )
    await gotoRuntimeSettings(page, baseURL)
    const card = installedCard(page)
    await expect(card.getByText(/E2E e2e-gate-/)).toBeVisible({ timeout: 15000 })
    await expect(card.getByRole('button', { name: 'Start' })).toHaveCount(0)

    // + manage: the Start control appears.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead, Permissions.LocalRuntimeManage],
      'lrt-manage'
    )
    await gotoRuntimeSettings(page, baseURL)
    await expect(
      installedCard(page).getByRole('button', { name: 'Start' }).first()
    ).toBeVisible({ timeout: 15000 })
  })

  test('logs gates the Logs control independently of manage', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // Admin downloads a real engine + GGUF and starts the model so the row
    // exposes the running-state controls (Stop/Restart/Logs).
    await loginAsAdmin(page, baseURL)
    const adminToken = await getCurrentUserToken(page)
    await downloadEngineViaApi(baseURL, adminToken, 'llamacpp', 'v0.0.1-alpha', true)
    const providerId = await seedLocalProvider(baseURL, adminToken)
    await downloadGgufModelViaApi(baseURL, adminToken, providerId) // ~670 MB

    await gotoRuntimeSettings(page, baseURL)
    const card = installedCard(page)
    await expect(card.getByText('E2E TinyLlama')).toBeVisible({ timeout: 30000 })
    const startBtn = card.getByRole('button', { name: 'Start' })
    if (await startBtn.isVisible().catch(() => false)) {
      await startBtn.click()
    }
    await expect(card.getByRole('button', { name: 'Stop' }).first()).toBeVisible({
      timeout: 180000
    })

    // logs-only (read + versions_read + logs, NO manage): Logs shown, Stop hidden.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead, Permissions.LocalRuntimeLogs],
      'lrt-logsonly'
    )
    await gotoRuntimeSettings(page, baseURL)
    const c1 = installedCard(page)
    await expect(c1.getByText('E2E TinyLlama')).toBeVisible({ timeout: 15000 })
    await expect(c1.getByRole('button', { name: 'Logs' }).first()).toBeVisible()
    await expect(c1.getByRole('button', { name: 'Stop' })).toHaveCount(0)

    // manage-without-logs: Stop/Restart shown, Logs hidden.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead, Permissions.LocalRuntimeManage],
      'lrt-managenologs'
    )
    await gotoRuntimeSettings(page, baseURL)
    const c2 = installedCard(page)
    await expect(c2.getByRole('button', { name: 'Stop' }).first()).toBeVisible({ timeout: 15000 })
    await expect(c2.getByRole('button', { name: 'Logs' })).toHaveCount(0)
  })
})
