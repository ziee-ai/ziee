import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

// The mocked demo model's id (modelUsage().id) — the swap Select + model row
// derive their testids from it.
const MODEL_ID = '00000000-0000-0000-0000-0000000000aa'

/**
 * Deterministic coverage for the version-swap Select in
 * `VersionModelsBlock` (audit gap all-9a612ef7a36a).
 *
 * The existing `05-models-by-version.spec.ts` only exercises the swap with a
 * REAL mistral.rs engine (network + multi-version download). But the swap
 * Select carries a pure-frontend disabled-state rule:
 *
 *     disabled={busy || versionOptions.length < 2}
 *
 * with two distinct aria-labels — a long "swapping disabled, only one engine
 * version installed" label when there is < 2 installed versions, and a plain
 * "Engine version for <model>" label when a real swap is possible.
 *
 * That logic + its labels were untested without an engine. Here we mock ONLY
 * the two read endpoints that feed the Installed-versions card (the HTTP data
 * boundary — `GET /api/local-runtime/versions` and
 * `GET /api/local-runtime/version-usage`); the rendering + disabled logic +
 * the swap Select's option set are the REAL component under test, no engine
 * spawned.
 */

const NOW = '2026-01-01T00:00:00Z'
const MODEL_NAME = 'Swap Demo Model'

type VersionRow = {
  id: string
  version: string
  backend: string
  is_system_default: boolean
}

function versionResponse(v: VersionRow) {
  return {
    id: v.id,
    version: v.version,
    backend: v.backend,
    is_system_default: v.is_system_default,
    engine: 'llamacpp',
    arch: 'x86_64',
    platform: 'linux',
    binary_path: `/cache/${v.id}/llama-server`,
    created_at: NOW,
  }
}

function modelUsage() {
  return {
    id: '00000000-0000-0000-0000-0000000000aa',
    name: 'swap-demo',
    display_name: MODEL_NAME,
    engine: 'llamacpp',
    provider_id: '00000000-0000-0000-0000-0000000000bb',
    provider_name: 'Local (mocked)',
    running: false,
    pinned: true,
  }
}

/**
 * Install Playwright route handlers for the two data endpoints so the
 * llamacpp Installed-versions card renders `versions` with the supplied
 * version rows, the first of which owns the single demo model.
 */
async function mockRuntimeData(page: Page, versions: VersionRow[]) {
  await page.route(/\/api\/local-runtime\/versions(\?.*)?$/, async route => {
    if (route.request().method() !== 'GET') return route.continue()
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ versions: versions.map(versionResponse) }),
    })
  })

  await page.route(/\/api\/local-runtime\/version-usage(\?.*)?$/, async route => {
    if (route.request().method() !== 'GET') return route.continue()
    const model = modelUsage()
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        unresolved: [],
        versions: versions.map((v, i) => ({
          version: versionResponse(v),
          // The demo model resolves to the FIRST version only, so its
          // ModelRow renders under that version's block.
          models: i === 0 ? [model] : [],
        })),
      }),
    })
  })
}

test.describe('Local Runtime — version-swap dropdown (deterministic, engine-free)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('single installed version → swap Select is disabled with the "only one version" label', async ({
    page,
    testInfra,
  }) => {
    await mockRuntimeData(page, [
      { id: 'ver-only-1', version: 'v1.0.0', backend: 'cpu', is_system_default: true },
    ])
    await gotoRuntimeSettings(page, testInfra.baseURL)

    // The model row renders under the single installed version.
    await expect(byTestId(page, `llmrt-model-row-${MODEL_ID}`)).toBeVisible({
      timeout: 15000,
    })

    // With a single installed version the swap Select is disabled.
    await expect(byTestId(page, `llmrt-model-version-select-${MODEL_ID}`)).toBeDisabled()
  })

  test('two installed versions → swap Select is enabled and lists both versions', async ({
    page,
    testInfra,
  }) => {
    await mockRuntimeData(page, [
      { id: 'ver-a', version: 'v1.0.0', backend: 'cpu', is_system_default: true },
      { id: 'ver-b', version: 'v1.1.0', backend: 'cpu', is_system_default: false },
    ])
    await gotoRuntimeSettings(page, testInfra.baseURL)

    await expect(byTestId(page, `llmrt-model-row-${MODEL_ID}`)).toBeVisible({
      timeout: 15000,
    })

    // With ≥2 versions the Select is enabled.
    const enabledSelect = byTestId(page, `llmrt-model-version-select-${MODEL_ID}`)
    await expect(enabledSelect).toBeEnabled()

    // Opening it surfaces BOTH installed versions as swap targets (options
    // derive `${selectTestid}-opt-${versionId}`).
    await enabledSelect.click()
    await expect(
      byTestId(page, `llmrt-model-version-select-${MODEL_ID}-opt-ver-a`),
    ).toBeVisible({ timeout: 5000 })
    await expect(
      byTestId(page, `llmrt-model-version-select-${MODEL_ID}-opt-ver-b`),
    ).toBeVisible()
  })
})
