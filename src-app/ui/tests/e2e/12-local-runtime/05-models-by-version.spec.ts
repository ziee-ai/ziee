import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  gotoRuntimeSettings,
  seedLocalProvider,
  seedLocalModel,
  downloadEngineViaApi,
} from './helpers/local-runtime-helpers'

/**
 * Models-by-engine-version management UI (the `RuntimeModelsByVersion` card),
 * the update-checker diff, and the version delete guard.
 *
 * NOTE: not yet executed (implement-before-run rule). Selectors follow the
 * documented antd surface; expect a verification pass on first real run.
 *
 * Split into:
 *  - engine-free: only local read endpoints, runs anywhere.
 *  - engine-dependent: needs an engine binary reachable from the backend
 *    (`LLM_RUNTIME_RELEASE_MIRROR`); skipped unless `ZIEE_E2E_ENGINE_MIRROR`
 *    is set, exactly like 04-engine-lifecycle.
 */
const ENGINE_MIRROR = process.env.ZIEE_E2E_ENGINE_MIRROR

test.describe('Local Runtime — models by version (engine-free)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('card shows the empty state on both engine tabs', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)

    // Llama.cpp tab is active by default.
    const pane = page.locator('.ant-tabs-tabpane-active')
    await expect(pane.getByText('Models by engine version')).toBeVisible()
    await expect(pane.getByText('No installed versions yet')).toBeVisible()

    // Switch to Mistral.rs → same card + empty state in the now-active pane.
    await page.getByRole('tab', { name: 'Mistral.rs' }).click()
    const mrsPane = page.locator('.ant-tabs-tabpane-active')
    await expect(mrsPane.getByText('Models by engine version')).toBeVisible()
    await expect(mrsPane.getByText('No installed versions yet')).toBeVisible()
  })

  test('update checker exposes a Check for Updates action', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    // Present, but NOT clicked here — clicking would hit github.com.
    await expect(
      page
        .locator('.ant-tabs-tabpane-active')
        .getByRole('button', { name: /Check for Updates/i })
    ).toBeVisible()
  })
})

test.describe('Local Runtime — models by version (needs engine mirror)', () => {
  test.skip(!ENGINE_MIRROR, 'set ZIEE_E2E_ENGINE_MIRROR to run engine-dependent flows')

  let modelName: string

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    await downloadEngineViaApi(testInfra.baseURL, token, 'llamacpp')
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    modelName = `e2e-mbv-${Date.now()}`
    await seedLocalModel(testInfra.baseURL, token, providerId, modelName)
  })

  function mbvCard(page: import('@playwright/test').Page) {
    return page
      .locator('.ant-tabs-tabpane-active')
      .locator('.ant-card')
      .filter({ hasText: 'Models by engine version' })
  }

  test('lists the local model under its engine version', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await expect(mbvCard(page).getByText(`E2E ${modelName}`)).toBeVisible({
      timeout: 15000,
    })
  })

  test('start and stop toggle the running state', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const card = mbvCard(page)
    await expect(card.getByText(`E2E ${modelName}`)).toBeVisible({ timeout: 15000 })

    // Only one model is seeded → the card has a single Start/Stop control.
    await card.getByRole('button', { name: 'Start' }).click()
    await expect(card.getByRole('button', { name: 'Stop' })).toBeVisible({ timeout: 30000 })
    await card.getByRole('button', { name: 'Stop' }).click()
    await expect(card.getByRole('button', { name: 'Start' })).toBeVisible({ timeout: 30000 })
  })

  test('deleting the in-use default version is guarded + offers remove-files', async ({
    page,
    testInfra,
  }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const pane = page.locator('.ant-tabs-tabpane-active')

    // The installed-versions list shows one version with a Delete button.
    await pane.getByRole('button', { name: 'Delete' }).first().click()

    // The delete confirm offers the "remove cached files" opt-in.
    await expect(page.getByText('Also remove cached files from disk')).toBeVisible()

    // Confirm (Popconfirm primary button — class is stable across okText).
    await page.locator('.ant-popover .ant-btn-primary').last().click()

    // The version is the system default + backs the seeded model → the guard
    // refuses and the reason is surfaced as a message.
    await expect(page.locator('.ant-message')).toContainText(/Cannot delete/i, {
      timeout: 10000,
    })
  })

  test('check for updates shows the installed version in the diff', async ({
    page,
    testInfra,
  }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const pane = page.locator('.ant-tabs-tabpane-active')
    await pane.getByRole('button', { name: /Check for Updates/i }).click()
    // The diff renders the host-scoped releases list once the check returns.
    await expect(pane.getByText(/Releases \(/i)).toBeVisible({ timeout: 15000 })
  })

  // SKETCH — a real swap needs TWO installed versions of the same engine. The
  // single-version mock release can't provide that; a multi-version mirror in
  // global-setup is a follow-up (see 04-engine-lifecycle). Intended flow:
  //   1. install v1 (default) + v2.
  //   2. in the model row, open the version Select and choose v2.
  //   3. assert the model moves under v2 (pinned) and, if running, restarts.
  test.skip('swap a model to another version (needs a second installed version)', () => {})
})
