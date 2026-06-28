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
 * E2E — HardwareSettings.renderGPUCards branches (HardwareSettings.tsx:231-300):
 * the no-GPU empty state, multiple GPU cards, and vendor/driver fields. The
 * hardware-info endpoint (GET /api/hardware) is the external boundary — mocked
 * to drive each rendering variation deterministically (real detection can't
 * produce a no-GPU / multi-GPU box on demand). The usage SSE is stubbed off.
 */

const BASE_HW = {
  cpu: { architecture: 'x86_64', cores: 8, model: 'Test CPU', threads: 16 },
  memory: { total_ram: 16 * 1024 * 1024 * 1024 },
  operating_system: { architecture: 'x86_64', name: 'Linux', version: '6.8' },
}

const CAPS = {
  cuda_support: false,
  metal_support: false,
  opencl_support: true,
}

async function mockHardware(page: import('@playwright/test').Page, gpu_devices: unknown[]) {
  await page.route(/\/api\/hardware$/, async route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ hardware: { ...BASE_HW, gpu_devices } }),
    }),
  )
  // Keep the page from establishing the live usage stream.
  await page.route(/\/api\/hardware\/usage-stream/, async route =>
    route.fulfill({ status: 500, contentType: 'text/plain', body: 'no stream' }),
  )
}

test.describe('Hardware — GPU rendering variations', () => {
  test('no GPU devices → the empty state renders', async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await mockHardware(page, [])
    await page.goto(`${testInfra.baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    await expect(page.getByText('No GPU devices detected')).toBeVisible({ timeout: 15000 })
  })

  test('multiple GPUs (NVIDIA + AMD) → a card per device with vendor/driver', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await mockHardware(page, [
      {
        device_id: 'gpu-0',
        name: 'NVIDIA RTX 4090',
        vendor: 'NVIDIA',
        memory: 24 * 1024 * 1024 * 1024,
        driver_version: '550.54',
        compute_capabilities: { ...CAPS, cuda_support: true, cuda_version: '12.4' },
      },
      {
        device_id: 'gpu-1',
        name: 'AMD Radeon RX 7900',
        vendor: 'AMD',
        memory: 20 * 1024 * 1024 * 1024,
        driver_version: '23.40',
        compute_capabilities: { ...CAPS, vulkan_support: true },
      },
    ])
    await page.goto(`${testInfra.baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // Both GPU cards render, titled by device name, with their vendor + driver.
    await expect(page.getByText('NVIDIA RTX 4090')).toBeVisible({ timeout: 15000 })
    await expect(page.getByText('AMD Radeon RX 7900')).toBeVisible()
    await expect(page.getByText('NVIDIA', { exact: true })).toBeVisible()
    await expect(page.getByText('AMD', { exact: true })).toBeVisible()
    await expect(page.getByText('550.54')).toBeVisible()
  })
})
