import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import { seedLocalProvider, seedLocalModel } from './helpers/local-runtime-helpers'
import { openEditModelDrawer } from '../05-llm/helpers/model-helpers'

/**
 * Local-model engine-settings form (EditLlmModelDrawer). This change
 * re-enabled the engine-specific settings sections, deduped the duplicate
 * component pairs, and pruned the deferred/unwired fields. These specs assert
 * the right section renders per engine and that the values round-trip through
 * the API (the bug being fixed: settings declared in the API/UI never reached
 * the backend's stored ModelEngineSettings).
 *
 * NOTE: authored alongside the backend work and NOT yet executed (per the
 * implement-before-run rule). Selectors follow the documented antd surface;
 * expect a verification pass (selector/timing tweaks) on the first real run.
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
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    // llama.cpp-specific cards render → the section is wired for local models.
    await expect(drawer.getByText('Context & Memory Management')).toBeVisible()
    await expect(drawer.getByText('GPU Configuration')).toBeVisible()
    // A mistral.rs-only card must NOT appear for a llamacpp model.
    await expect(drawer.getByText('PagedAttention Configuration')).toHaveCount(0)

    // Set ctx_size and save. Target by the ctx_size InputNumber's unique
    // placeholder ("8192") — the prior `.ant-flex(hasText 'Context Size')`
    // selector also matched the card's OUTER flex (which contains every
    // field), so `.last()` filled the card's last field, leaving ctx_size
    // untouched.
    const ctxInput = drawer.locator('input[placeholder="8192"]')
    await ctxInput.fill('8192')
    await ctxInput.blur()
    await drawer
      .locator('.ant-btn-primary[type="submit"], .ant-btn-primary')
      .last()
      .click()
    await page.getByText('Model updated successfully').waitFor({ timeout: 15000 })

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
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    await expect(drawer.getByText('PagedAttention Configuration')).toBeVisible()
    await expect(drawer.getByText('Sequence & Memory Management')).toBeVisible()
    // Pruned (deferred / unwired) fields must be gone.
    await expect(drawer.getByText('Vision Model Settings')).toHaveCount(0)
    await expect(drawer.getByText('Prompt Chunk Size')).toHaveCount(0)
    await expect(drawer.getByText('Max Sequence Length')).toHaveCount(0)
  })

  test('llamacpp persists a COMBINATION of engine fields together', async ({
    page,
    testInfra,
  }) => {
    // The existing test persists a single field (ctx_size). This exercises a
    // per-engine field COMBINATION (ctx_size + batch_size) round-tripping in one
    // save — guarding against only the last-edited field reaching the backend.
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
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    // Unique placeholders: ctx_size → "8192", batch_size → "2048".
    const ctx = drawer.locator('input[placeholder="8192"]')
    await ctx.fill('4096')
    await ctx.blur()
    const batch = drawer.locator('input[placeholder="2048"]')
    await batch.fill('1024')
    await batch.blur()

    await drawer
      .locator('.ant-btn-primary[type="submit"], .ant-btn-primary')
      .last()
      .click()
    await page.getByText('Model updated successfully').waitFor({ timeout: 15000 })

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
    // ctx_size / batch_size are covered above; the GPU Configuration card's
    // n_gpu_layers field (placeholder "0", non-unique → addressed via its
    // "GPU Layers" label) was untested.
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getCurrentUserToken(page)
    const providerId = await seedLocalProvider(testInfra.baseURL, token)
    const name = `e2e-lc-gpu-${Date.now()}`
    const modelId = await seedLocalModel(testInfra.baseURL, token, providerId, name, 'llamacpp')

    await page.goto(`${testInfra.baseURL}/settings/llm-providers/${providerId}`)
    await page.waitForLoadState('load')
    await openEditModelDrawer(page, `E2E ${name}`)
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    // The n_gpu_layers InputNumber lives in the "GPU Layers" ResponsiveConfigItem
    // (placeholder "0" is shared, so scope by the label's ancestor Flex).
    const gpu = drawer
      .getByText('GPU Layers', { exact: true })
      .locator('xpath=../..')
      .getByRole('spinbutton')
    await gpu.fill('24')
    await gpu.blur()

    await drawer.locator('.ant-btn-primary[type="submit"], .ant-btn-primary').last().click()
    await page.getByText('Model updated successfully').waitFor({ timeout: 15000 })

    const res = await page.request.get(
      `${testInfra.baseURL}/api/llm-models/${modelId}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(res.ok()).toBeTruthy()
    const model = await res.json()
    expect(model.engine_settings?.llamacpp?.n_gpu_layers).toBe(24)
  })
})
