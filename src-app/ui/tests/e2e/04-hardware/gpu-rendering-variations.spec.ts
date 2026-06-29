import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
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
