import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { seedLocalProvider, seedLocalModel } from './helpers/local-runtime-helpers'
import { openEditModelDrawer } from '../llm/helpers/model-helpers'

/**
 * Local-model engine-settings form (EditLlmModelDrawer). These specs assert
 * the right per-engine section renders and that values round-trip through the
 * API (the bug being fixed: settings declared in the API/UI never reached the
 * backend's stored ModelEngineSettings).
 *
 * Run with `--workers=1`.
 */

test.describe('Local Runtime — model engine settings form', () => {
  test('llamacpp model shows the llama.cpp cards and persists ctx_size', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const name = `e2e-lc-${Date.now()}`
    const modelId = await seedLocalModel(
      testInfra.baseURL,
      token,
      providerId,
      name,
      'llamacpp',
    )

    await page.goto(`${testInfra.baseURL}/settings/llm-providers/${providerId}`)
    await page.waitForLoadState('load')
    await openEditModelDrawer(page, `E2E ${name}`)
    const drawer = byTestId(page, 'llm-edit-model-form')

    // llama.cpp-specific cards render → the section is wired for local models.
    await expect(byTestId(drawer, 'llamacpp-card-context-memory')).toBeVisible()
    await expect(byTestId(drawer, 'llamacpp-card-gpu')).toBeVisible()
    // A mistral.rs-only card must NOT appear for a llamacpp model.
    await expect(byTestId(drawer, 'mistralrs-card-paged-attention')).toHaveCount(0)

    // Set ctx_size and save.
    const ctxInput = byTestId(drawer, 'llm-llamacpp-ctx-size')
    await ctxInput.fill('8192')
    await ctxInput.blur()
    await byTestId(drawer, 'llm-edit-model-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 15000 })

    // Persistence (robust API check): the stored ModelEngineSettings carries
    // the nested value the form submitted.
    const res = await page.request.get(
      `${testInfra.baseURL}/api/llm-models/${modelId}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(res.ok()).toBeTruthy()
    const model = await res.json()
    expect(model.engine_settings?.llamacpp?.ctx_size).toBe(8192)
  })

  test('mistralrs model shows the mistral.rs cards without the pruned fields', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const name = `e2e-mrs-${Date.now()}`
    await seedLocalModel(testInfra.baseURL, token, providerId, name, 'mistralrs')

    await page.goto(`${testInfra.baseURL}/settings/llm-providers/${providerId}`)
    await page.waitForLoadState('load')
    await openEditModelDrawer(page, `E2E ${name}`)
    const drawer = byTestId(page, 'llm-edit-model-form')

    await expect(byTestId(drawer, 'mistralrs-card-paged-attention')).toBeVisible()
    await expect(byTestId(drawer, 'mistralrs-card-sequence-memory')).toBeVisible()
    // Pruned (deferred / unwired) fields must be gone.
    await expect(byTestId(drawer, 'mistralrs-card-vision')).toHaveCount(0)
    await expect(byTestId(drawer, 'llm-mistralrs-prompt-chunk-size')).toHaveCount(0)
    await expect(byTestId(drawer, 'llm-mistralrs-max-seq-len')).toHaveCount(0)
  })

  test('llamacpp persists a COMBINATION of engine fields together', async ({
    page,
    testInfra,
  }) => {
    // Exercises a per-engine field COMBINATION (ctx_size + batch_size)
    // round-tripping in one save — guarding against only the last-edited field
    // reaching the backend.
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const name = `e2e-lc-combo-${Date.now()}`
    const modelId = await seedLocalModel(
      testInfra.baseURL,
      token,
      providerId,
      name,
      'llamacpp',
    )

    await page.goto(`${testInfra.baseURL}/settings/llm-providers/${providerId}`)
    await page.waitForLoadState('load')
    await openEditModelDrawer(page, `E2E ${name}`)
    const drawer = byTestId(page, 'llm-edit-model-form')

    const ctx = byTestId(drawer, 'llm-llamacpp-ctx-size')
    await ctx.fill('4096')
    await ctx.blur()
    const batch = byTestId(drawer, 'llm-llamacpp-batch-size')
    await batch.fill('1024')
    await batch.blur()

    await byTestId(drawer, 'llm-edit-model-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 15000 })

    // BOTH fields must persist (not just the last-edited one).
    const res = await page.request.get(
      `${testInfra.baseURL}/api/llm-models/${modelId}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(res.ok()).toBeTruthy()
    const model = await res.json()
    expect(model.engine_settings?.llamacpp?.ctx_size).toBe(4096)
    expect(model.engine_settings?.llamacpp?.batch_size).toBe(1024)
  })

  test('llamacpp persists n_gpu_layers from the GPU Configuration card', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const name = `e2e-lc-gpu-${Date.now()}`
    const modelId = await seedLocalModel(testInfra.baseURL, token, providerId, name, 'llamacpp')

    await page.goto(`${testInfra.baseURL}/settings/llm-providers/${providerId}`)
    await page.waitForLoadState('load')
    await openEditModelDrawer(page, `E2E ${name}`)
    const drawer = byTestId(page, 'llm-edit-model-form')

    const gpu = byTestId(drawer, 'llm-llamacpp-n-gpu-layers')
    await gpu.fill('24')
    await gpu.blur()

    await byTestId(drawer, 'llm-edit-model-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 15000 })

    const res = await page.request.get(
      `${testInfra.baseURL}/api/llm-models/${modelId}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(res.ok()).toBeTruthy()
    const model = await res.json()
    expect(model.engine_settings?.llamacpp?.n_gpu_layers).toBe(24)
  })
})
