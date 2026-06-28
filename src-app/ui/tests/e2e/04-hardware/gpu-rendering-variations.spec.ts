import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — HardwareSettings.renderGPUCards branches (no-GPU vs multi-GPU).
 *
 * Audit gap: the GPU rendering branches were never exercised because CI
 * hosts have one (or zero) real GPUs. This intercepts GET /api/hardware,
 * fetches the REAL response (so every other field stays valid) and overrides
 * ONLY `hardware.gpu_devices` — the external boundary — to drive the
 * empty-array ("No GPU devices detected") and the multi-GPU (one card per
 * device) branches.
 */

const HARDWARE = '**/api/hardware'

function fakeGpu(id: string, name: string, vendor: string, memory: number) {
  return {
    device_id: id,
    name,
    vendor,
    memory,
    driver_version: '1.0',
    compute_capabilities: {
      cuda_support: vendor.includes('NVIDIA'),
      metal_support: vendor.includes('Apple'),
      opencl_support: true,
    },
  }
}

async function routeWithGpus(
  page: import('@playwright/test').Page,
  gpus: unknown[],
) {
  await page.route(HARDWARE, async route => {
    if (route.request().method() !== 'GET') return route.fallback()
    const resp = await route.fetch()
    const body = await resp.json()
    body.hardware.gpu_devices = gpus
    await route.fulfill({ response: resp, json: body })
  })
}

test.describe('Hardware — GPU rendering variations', () => {
  test('no GPU devices → "No GPU devices detected"', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await routeWithGpus(page, [])

    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    await expect(page.getByText('No GPU devices detected')).toBeVisible({
      timeout: 30000,
    })
  })

  test('multiple GPUs → one card per device', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await routeWithGpus(page, [
      fakeGpu('gpu-0', 'NVIDIA H200 #0', 'NVIDIA', 150_000_000_000),
      fakeGpu('gpu-1', 'NVIDIA H200 #1', 'NVIDIA', 150_000_000_000),
    ])

    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    await expect(page.getByText('NVIDIA H200 #0')).toBeVisible({
      timeout: 30000,
    })
    await expect(page.getByText('NVIDIA H200 #1')).toBeVisible()
  })
})
