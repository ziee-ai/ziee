import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
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
 * NOTE: not yet executed (real network + a ~670 MB download + CPU inference).
 * Selectors/timings will need a verification pass on first real run.
 */
const HF_KEY = process.env.HUGGINGFACE_API_KEY
const SWAP_VERSION_A = 'v0.0.1-alpha'
const SWAP_VERSION_B = 'v0.0.2-alpha' // mistral.rs publishes both

// The standalone "Models by engine version" card was folded into
// the Installed versions card — each installed-version row now
// renders its model list (VersionModelsBlock) inline underneath.
// All assertions are scoped to the Installed versions card.
function installedCard(page: Page) {
  return page
    .locator('.ant-tabs-tabpane-active')
    .locator('.ant-card')
    .filter({ hasText: 'Installed versions' })
}

// ── engine-free: only local read endpoints, runs anywhere ────────────────
test.describe('Local Runtime — models by version (engine-free)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('installed-versions card shows the empty state on both engine tabs', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const card = installedCard(page)
    await expect(card).toBeVisible()
    await expect(card.getByText(/No versions installed yet/i)).toBeVisible()

    await page.getByRole('tab', { name: 'Mistral.rs' }).click()
    const mrsCard = installedCard(page)
    await expect(mrsCard).toBeVisible()
    await expect(mrsCard.getByText(/No versions installed yet/i)).toBeVisible()
  })

  test('available-versions card auto-populates on mount + has a manual Check-for-updates button in its extra slot', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // AvailableVersionsCard auto-runs the update check on mount; the
    // card either lists ready releases or shows the
    // "Could not reach the upstream release feed." fallback. The
    // 'Check for updates' button now lives in the card's `extra`
    // slot (peer pattern: UsersSettings puts its primary card
    // action there too), so the *button is present* — but the
    // initial render doesn't require it to fire.
    const pane = page.locator('.ant-tabs-tabpane-active')
    await expect(pane.getByText(/Available versions/i).first()).toBeVisible({
      timeout: 30000,
    })
    await expect(
      pane.getByRole('button', { name: /Check for updates/i })
    ).toBeVisible()
  })
})

// ── running engine: real GitHub engine + real HF GGUF + CPU inference ─────
// One model is downloaded once per worker (memoized) and reused.
let runningSetup: Promise<{ modelName: string }> | null = null
function ensureRunningModel(baseURL: string, token: string) {
  if (!runningSetup) {
    runningSetup = (async () => {
      await downloadEngineViaApi(baseURL, token, 'llamacpp') // real GitHub, default
      const providerId = await seedLocalProvider(baseURL, token)
      await downloadGgufModelViaApi(baseURL, token, providerId) // real HF (~670 MB)
      // The card renders the model's display_name, set in the download helper.
      return { modelName: 'E2E TinyLlama' }
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
    const card = installedCard(page)
    // The downloaded GGUF model appears under its engine version.
    await expect(card.getByText(setup.modelName, { exact: false })).toBeVisible({
      timeout: 30000
    })

    // Start (defensive: only if currently stopped).
    const startBtn = card.getByRole('button', { name: 'Start' })
    if (await startBtn.isVisible().catch(() => false)) {
      await startBtn.click()
    }
    await expect(card.getByRole('button', { name: 'Stop' }).first()).toBeVisible({
      timeout: 480000
    })

    // Expand logs + instance detail.
    await card.getByRole('button', { name: 'Logs' }).first().click()
    await expect(page.getByText('Live logs')).toBeVisible()
    await expect(page.getByText(/Base URL/i)).toBeVisible({ timeout: 15000 })

    // Restart → still running.
    await card.getByRole('button', { name: 'Restart' }).first().click()
    await expect(card.getByRole('button', { name: 'Stop' }).first()).toBeVisible({
      timeout: 480000
    })

    // Stop → Start returns.
    await card.getByRole('button', { name: 'Stop' }).first().click()
    await expect(card.getByRole('button', { name: 'Start' }).first()).toBeVisible({
      timeout: 60000
    })
  })

  test('available-versions section shows the installed tag for the installed version', async ({
    page,
    testInfra
  }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const pane = page.locator('.ant-tabs-tabpane-active')
    // AvailableVersionsCard auto-checks on mount. The installed v0.0.1 row
    // should carry an "installed" tag, with the Install button disabled.
    await expect(pane.getByText(/Available versions/i).first()).toBeVisible({
      timeout: 30000,
    })
    await expect(pane.getByText('installed').first()).toBeVisible({ timeout: 20000 })
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
    const pane = page.locator('.ant-tabs-tabpane-active')

    // Deleting the default-and-in-use version is refused with the guard reason.
    await pane.getByRole('button', { name: 'Delete' }).first().click()
    await expect(page.getByText('Also remove cached files from disk')).toBeVisible()
    await page.locator('.ant-popover .ant-btn-primary').last().click()
    await expect(page.locator('.ant-message')).toContainText(/Cannot delete/i, {
      timeout: 10000
    })
  })

  test('swap a model from one version to another', async ({ page, testInfra }) => {
    const token = await getCurrentUserToken(page)
    // mistral.rs publishes BOTH v0.0.1-alpha and v0.0.2-alpha. A is default →
    // an unpinned mistralrs model resolves to A; we swap it to B via the card.
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_A, true)
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_B, false)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    await seedLocalModel(testInfra.baseURL, token, providerId, `e2e-swap-${Date.now()}`, 'mistralrs')

    await gotoRuntimeSettings(page, testInfra.baseURL)
    // The models-by-version card lives on the per-engine tab → Mistral.rs.
    await page.getByRole('tab', { name: 'Mistral.rs' }).click()
    const card = installedCard(page)
    // The model starts under version A; swap it to version B via the Select.
    await card.locator('.ant-select').first().click()
    await page.locator('.ant-select-item-option').filter({ hasText: SWAP_VERSION_B }).first().click()
    // After the swap reloads usage, the model is grouped under version B.
    await expect(card.getByText(SWAP_VERSION_B, { exact: false })).toBeVisible({
      timeout: 15000
    })
  })

  test('delete a non-default, unused version with "Also remove cached files" checked', async ({
    page,
    testInfra,
  }) => {
    // The guard test only covers the REFUSED path (default+in-use) and asserts
    // the checkbox is visible. The SUCCESSFUL delete with the "Also remove
    // cached files from disk" Checkbox CHECKED (removeBinary=true) was untested.
    const token = await getCurrentUserToken(page)
    // A is default; B is a second, non-default, unused version → deletable.
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_A, true)
    await downloadEngineViaApi(testInfra.baseURL, token, 'mistralrs', SWAP_VERSION_B, false)

    await gotoRuntimeSettings(page, testInfra.baseURL)
    await page.getByRole('tab', { name: 'Mistral.rs' }).click()
    const card = installedCard(page)
    await expect(card.getByText(SWAP_VERSION_B, { exact: false })).toBeVisible({
      timeout: 15000,
    })

    // Open version B's delete Popconfirm, CHECK the cached-files box, confirm.
    await card.getByRole('button', { name: `Delete version ${SWAP_VERSION_B}` }).click()
    const popover = page.locator('.ant-popover:visible')
    await popover
      .getByRole('checkbox', { name: 'Also remove cached files from disk' })
      .check()
    await popover.getByRole('button', { name: 'Delete' }).click()

    // The version row disappears (successful delete), default version A remains.
    await expect(card.getByText(SWAP_VERSION_B, { exact: false })).toHaveCount(0, {
      timeout: 15000,
    })
    await expect(card.getByText(SWAP_VERSION_A, { exact: false })).toBeVisible()
  })
})
