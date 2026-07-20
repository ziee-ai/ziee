import { test, expect } from '../../fixtures/test-context'
import type { Page, Locator } from '@playwright/test'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid.ts'
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
 * principal only sees the affordances for permissions it actually holds.
 *
 * Covered:
 *  - `versions_read` gates the version-catalogue / update-checker / per-version
 *    usage sections (the page route only requires `read`).
 *  - `manage` gates the model start/stop/restart/swap controls.
 *  - `logs` gates the Logs control INDEPENDENTLY of `manage`.
 */
const HF_KEY = process.env.HUGGINGFACE_API_KEY

// Baseline reads so the settings page renders without an unrelated card (the
// runtime-config card reads `settings_read`) erroring; `versions_read` is then
// the only variable for the version sections.
const BASE_READS = [Permissions.LocalRuntimeRead, Permissions.RuntimeSettingsRead]

// The Installed versions card is the row-level surface for model
// start/stop/swap controls now. Engine-backed permission tests scope
// assertions to this per-engine card.
function installedCard(page: Page, engine: 'llamacpp' | 'mistralrs' = 'llamacpp'): Locator {
  return byTestId(page, `llmrt-installed-versions-card-${engine}`)
}

// ── engine-free: versions_read gates whole sections ──────────────────────
test.describe('Local Runtime — permission gating (engine-free)', () => {
  test('versions_read gates the installed-versions + available-versions cards', async ({
    page,
    testInfra
  }) => {
    const { baseURL, apiURL } = testInfra

    // read (no versions_read): page loads, but both per-engine cards are hidden.
    await loginWithPerms(page, baseURL, apiURL, BASE_READS, 'lrt-noversions')
    await gotoRuntimeSettings(page, baseURL)
    await expect(byTestId(page, 'llmrt-engine-tabs-tab-llamacpp')).toBeVisible()
    await expect(byTestId(page, 'llmrt-available-versions-card')).toHaveCount(0)
    await expect(byTestId(page, 'llmrt-installed-versions-card-llamacpp')).toHaveCount(0)
    await expect(byTestId(page, 'llmrt-check-updates-btn')).toHaveCount(0)

    // + versions_read: both cards appear, and the Available versions card
    // surfaces a manual "Check for updates" button in its `extra` slot.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead],
      'lrt-versions'
    )
    await gotoRuntimeSettings(page, baseURL)
    await expect(byTestId(page, 'llmrt-available-versions-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'llmrt-installed-versions-card-llamacpp')).toBeVisible()
    await expect(byTestId(page, 'llmrt-check-updates-btn')).toBeVisible()
  })

  test('settings_manage gates the Runtime configuration Save control', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // settings_read (no settings_manage): the Runtime configuration card renders
    // but its Save button is hidden + the form is disabled.
    await loginWithPerms(page, baseURL, apiURL, BASE_READS, 'lrt-cfg-nomanage')
    await gotoRuntimeSettings(page, baseURL)
    await expect(byTestId(page, 'llmrt-runtime-config-card')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'llmrt-config-save-btn')).toHaveCount(0)

    // + settings_manage: the Save control appears.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeSettingsManage],
      'lrt-cfg-manage',
    )
    await gotoRuntimeSettings(page, baseURL)
    await expect(byTestId(page, 'llmrt-runtime-config-card')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'llmrt-config-save-btn')).toBeVisible()
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
    const modelId = await seedLocalModel(baseURL, adminToken, providerId, `e2e-gate-${Date.now()}`)

    // read + versions_read, NO manage: model row visible, no Start button.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead],
      'lrt-nomanage'
    )
    await gotoRuntimeSettings(page, baseURL)
    const card = installedCard(page, 'llamacpp')
    await expect(byTestId(card, `llmrt-model-row-${modelId}`)).toBeVisible({ timeout: 15000 })
    await expect(byTestId(card, `llmrt-model-start-${modelId}`)).toHaveCount(0)

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
      byTestId(installedCard(page, 'llamacpp'), `llmrt-model-start-${modelId}`)
    ).toBeVisible({ timeout: 15000 })
  })

  test('logs gates the Logs control independently of manage', async ({ page, testInfra }) => {
    // Full real-engine spawn + cold-CPU first-token; bump the test budget.
    test.setTimeout(600000)
    const { baseURL, apiURL } = testInfra
    // Admin downloads a real engine + GGUF and starts the model so the row
    // exposes the running-state controls (Stop/Restart/Logs).
    await loginAsAdmin(page, baseURL)
    const adminToken = await getCurrentUserToken(page)
    await downloadEngineViaApi(baseURL, adminToken, 'llamacpp', 'v0.0.1-alpha', true)
    const providerId = await seedLocalProvider(baseURL, adminToken)
    const model = await downloadGgufModelViaApi(baseURL, adminToken, providerId) // ~670 MB
    const modelId = model.id

    await gotoRuntimeSettings(page, baseURL)
    const card = installedCard(page, 'llamacpp')
    await expect(byTestId(card, `llmrt-model-row-${modelId}`)).toBeVisible({ timeout: 30000 })
    const startBtn = byTestId(card, `llmrt-model-start-${modelId}`)
    if (await startBtn.isVisible().catch(() => false)) {
      await startBtn.click()
    }
    await expect(byTestId(card, `llmrt-model-stop-${modelId}`)).toBeVisible({
      timeout: 480000
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
    const c1 = installedCard(page, 'llamacpp')
    await expect(byTestId(c1, `llmrt-model-row-${modelId}`)).toBeVisible({ timeout: 15000 })
    await expect(byTestId(c1, `llmrt-model-logs-${modelId}`)).toBeVisible()
    await expect(byTestId(c1, `llmrt-model-stop-${modelId}`)).toHaveCount(0)

    // manage-without-logs: Stop/Restart shown, Logs hidden.
    await loginWithPerms(
      page,
      baseURL,
      apiURL,
      [...BASE_READS, Permissions.RuntimeVersionRead, Permissions.LocalRuntimeManage],
      'lrt-managenologs'
    )
    await gotoRuntimeSettings(page, baseURL)
    const c2 = installedCard(page, 'llamacpp')
    await expect(byTestId(c2, `llmrt-model-stop-${modelId}`)).toBeVisible({ timeout: 15000 })
    await expect(byTestId(c2, `llmrt-model-logs-${modelId}`)).toHaveCount(0)
  })
})
