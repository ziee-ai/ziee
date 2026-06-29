import { test, expect } from '../../fixtures/test-context'
import type { Page, Locator } from '@playwright/test'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import {
  gotoRuntimeSettings,
  seedLocalProvider,
  seedLocalModel,
  downloadEngineViaApi,
  downloadGgufModelViaApi
} from './helpers/local-runtime-helpers'

/**
 * Models-by-engine-version UI, the update-checker diff, version swap, and the
 * delete guard.
 *
 * Engine-dependent specs hit the REAL `ziee-ai/*` GitHub releases (small cpu
 * engine binary) and, for the running/lifecycle test, a REAL TinyLlama GGUF
 * from HuggingFace — the same path `gold_smoke` proves on the backend. They
 * are gated on `HUGGINGFACE_API_KEY` (source `server/tests/.env.test` before
 * `npm run test:e2e`); the backend inherits the key from the shell env.
 *
 * Run with `--workers=1`.
 */
const HF_KEY = process.env.HUGGINGFACE_API_KEY
const SWAP_VERSION_A = 'v0.0.1-alpha'
const SWAP_VERSION_B = 'v0.0.2-alpha' // mistral.rs publishes both

// The standalone "Models by engine version" card was folded into
// the Installed versions card — each installed-version row now
// renders its model list (VersionModelsBlock) inline underneath.
// All assertions are scoped to the per-engine Installed versions card.
function installedCard(page: Page, engine: 'llamacpp' | 'mistralrs' = 'llamacpp'): Locator {
  return byTestId(page, `llmrt-installed-versions-card-${engine}`)
}

// ── engine-free: only local read endpoints, runs anywhere ────────────────
test.describe('Local Runtime — models by version (engine-free)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('installed-versions card shows the empty state on both engine tabs', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const card = installedCard(page, 'llamacpp')
    await expect(card).toBeVisible()
    await expect(byTestId(card, 'llmrt-installed-empty-llamacpp')).toBeVisible()

    await byTestId(page, 'llmrt-engine-tabs-tab-mistralrs').click()
    const mrsCard = installedCard(page, 'mistralrs')
    await expect(mrsCard).toBeVisible()
    await expect(byTestId(mrsCard, 'llmrt-installed-empty-mistralrs')).toBeVisible()
  })

  test('available-versions card auto-populates on mount + has a manual Check-for-updates button in its extra slot', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // AvailableVersionsCard auto-runs the update check on mount; the
    // 'Check for updates' button lives in the card's `extra` slot.
    await expect(byTestId(page, 'llmrt-available-versions-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'llmrt-check-updates-btn')).toBeVisible()
  })
})

// ── running engine: real GitHub engine + real HF GGUF + CPU inference ─────
// One model is downloaded once per worker (memoized) and reused.
let runningSetup: Promise<{ modelName: string; modelId: string }> | null = null
function ensureRunningModel(baseURL: string, token: string) {
  if (!runningSetup) {
    runningSetup = (async () => {
      await downloadEngineViaApi(baseURL, token, 'llamacpp') // real GitHub, default
      const providerId = await seedLocalProvider(baseURL, token)
      const model = await downloadGgufModelViaApi(baseURL, token, providerId) // real HF (~670 MB)
      // The card renders the model's display_name, set in the download helper.
      return { modelName: 'E2E TinyLlama', modelId: model.id }
    })()
  }
  return runningSetup
}

test.describe('Local Runtime — running engine (needs HUGGINGFACE_API_KEY)', () => {
  test.skip(!HF_KEY, 'set HUGGINGFACE_API_KEY (source server/tests/.env.test) to run real-engine flows')

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    await ensureRunningModel(testInfra.baseURL, token)
  })

  test('full lifecycle: start → logs/detail → restart → stop', async ({ page, testInfra }) => {
    // Two cold-start cycles in this test (Start → Stop → Restart spawns
    // another). Cold-CPU first-token after spawn is slow on commodity
    // Macs; need ~8 min per spawn, so budget 16 min for the whole test.
    test.setTimeout(960000)
    const setup = await ensureRunningModel(testInfra.baseURL, await getCurrentUserToken(page))
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const card = installedCard(page, 'llamacpp')
    // The downloaded GGUF model appears under its engine version.
    await expect(byTestId(card, `llmrt-model-row-${setup.modelId}`)).toBeVisible({
      timeout: 30000
    })

    // Start (defensive: only if currently stopped).
    const startBtn = byTestId(card, `llmrt-model-start-${setup.modelId}`)
    if (await startBtn.isVisible().catch(() => false)) {
      await startBtn.click()
    }
    await expect(byTestId(card, `llmrt-model-stop-${setup.modelId}`)).toBeVisible({
      timeout: 480000
    })

    // Expand logs + instance detail.
    await byTestId(card, `llmrt-model-logs-${setup.modelId}`).click()
    await expect(byTestId(page, `llmrt-live-logs-card-${setup.modelId}`)).toBeVisible()
    await expect(byTestId(page, `llmrt-model-instance-desc-${setup.modelId}`)).toBeVisible({ timeout: 15000 })

    // Restart → still running.
    await byTestId(card, `llmrt-model-restart-${setup.modelId}`).click()
    await expect(byTestId(card, `llmrt-model-stop-${setup.modelId}`)).toBeVisible({
      timeout: 480000
    })

    // Stop → Start returns.
    await byTestId(card, `llmrt-model-stop-${setup.modelId}`).click()
    await expect(byTestId(card, `llmrt-model-start-${setup.modelId}`)).toBeVisible({
      timeout: 60000
    })
  })

  test('available-versions section shows the installed tag for the installed version', async ({
    page,
    testInfra
  }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // AvailableVersionsCard auto-checks on mount. The installed version row
    // should carry an "installed" tag (derived `llmrt-version-installed-tag-<ver>`).
    await expect(byTestId(page, 'llmrt-available-versions-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(
      page.locator('[data-testid^="llmrt-version-installed-tag-"]').first()
    ).toBeVisible({ timeout: 20000 })
  })
})

// ── version management: real engine(s) + a model ROW (no GGUF/inference) ──
test.describe('Local Runtime — version management (needs HUGGINGFACE_API_KEY)', () => {
  test.skip(!HF_KEY, 'set HUGGINGFACE_API_KEY to run real-engine flows')

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('delete guard: in-use/default version refused, empty version deletes', async ({
    page,
    testInfra
  }) => {
    const token = await getCurrentUserToken(page)
    // A single default version + a model row that resolves to it is enough to
    // exercise the in-use/default refusal (no second version needed).
    await downloadEngineViaApi(testInfra.baseURL, token, 'llamacpp', SWAP_VERSION_A, true)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    await seedLocalModel(testInfra.baseURL, token, providerId, `e2e-del-${Date.now()}`)

    await gotoRuntimeSettings(page, testInfra.baseURL)
    const card = installedCard(page, 'llamacpp')

    // Open the delete Confirm for the default-and-in-use version.
    await byTestId(card, `llmrt-version-delete-${SWAP_VERSION_A}`).click()
    // The cached-files option is present in the confirm dialog.
    await expect(byTestId(page, `llmrt-version-delete-removebinary-${SWAP_VERSION_A}`)).toBeVisible()
    // It's the system default → acknowledge to enable the confirm, then confirm.
    await byTestId(page, `llmrt-version-delete-ackdefault-${SWAP_VERSION_A}`).click()
    await byTestId(page, `llmrt-version-delete-confirm-${SWAP_VERSION_A}-confirm`).click()
    // The server refuses (in-use guard, 409) → error toast.
    await expect(page.locator('[data-sonner-toast]')).toContainText(/Cannot delete/i, {
      timeout: 10000
    })
  })

  test('swap a model from one version to another', async ({ page, testInfra }) => {
    const token = await getCurrentUserToken(page)
    // mistral.rs publishes BOTH v0.0.1-alpha and v0.0.2-alpha. A is default →
    // an unpinned mistralrs model resolves to A; we swap it to B via the card.
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_A, true)
    const vidB = await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_B, false)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const modelId = await seedLocalModel(testInfra.baseURL, token, providerId, `e2e-swap-${Date.now()}`, 'mistralrs')

    await gotoRuntimeSettings(page, testInfra.baseURL)
    // The models-by-version card lives on the per-engine tab → Mistral.rs.
    await byTestId(page, 'llmrt-engine-tabs-tab-mistralrs').click()
    const card = installedCard(page, 'mistralrs')
    // The model starts under version A; swap it to version B via the Select.
    await byTestId(card, `llmrt-model-version-select-${modelId}`).click()
    await byTestId(page, `llmrt-model-version-select-${modelId}-opt-${vidB}`).click()
    // After the swap reloads usage, the model row is grouped under version B's block.
    await expect(
      byTestId(byTestId(page, `llmrt-version-models-${vidB}`), `llmrt-model-row-${modelId}`)
    ).toBeVisible({ timeout: 15000 })
  })

  test('delete a non-default, unused version with "Also remove cached files" checked', async ({
    page,
    testInfra,
  }) => {
    const token = await getCurrentUserToken(page)
    // A is default; B is a second, non-default, unused version → deletable.
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_A, true)
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_B, false)

    await gotoRuntimeSettings(page, testInfra.baseURL)
    await byTestId(page, 'llmrt-engine-tabs-tab-mistralrs').click()
    const card = installedCard(page, 'mistralrs')
    await expect(byTestId(card, `llmrt-version-desc-${SWAP_VERSION_B}`)).toBeVisible({
      timeout: 15000,
    })

    // Open version B's delete Confirm, CHECK the cached-files box, confirm.
    await byTestId(card, `llmrt-version-delete-${SWAP_VERSION_B}`).click()
    await byTestId(page, `llmrt-version-delete-removebinary-${SWAP_VERSION_B}`).click()
    await byTestId(page, `llmrt-version-delete-confirm-${SWAP_VERSION_B}-confirm`).click()

    // The version row disappears (successful delete), default version A remains.
    await expect(byTestId(card, `llmrt-version-desc-${SWAP_VERSION_B}`)).toHaveCount(0, {
      timeout: 15000,
    })
    await expect(byTestId(card, `llmrt-version-desc-${SWAP_VERSION_A}`)).toBeVisible()
  })
})
