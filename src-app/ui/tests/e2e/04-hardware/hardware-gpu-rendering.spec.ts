import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// audit id all-c6a221168197 — renderGPUCards has distinct branches (no GPU vs N
// GPUs) that were untested. We mock GET /api/hardware (the external boundary)
// to drive each branch.
function hw(gpu_devices: unknown[]) {
  return {
    hardware: {
      operating_system: { name: 'Linux', version: 'test', kernel_version: 'x', arch: 'x86_64' },
      cpu: { brand: 'Test CPU', cores: 8, frequency: 3000 },
      memory: { total: 16 * 1024 * 1024 * 1024, used: 0, available: 16 * 1024 * 1024 * 1024 },
      gpu_devices,
    },
  }
}
async function mockHw(page: Page, gpu_devices: unknown[]) {
  await page.route(/\/api\/hardware$/, async (route, req) => {
    if (req.method() !== 'GET') return route.continue()
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(hw(gpu_devices)) })
  })
}

test.describe('Hardware — GPU rendering variations', () => {
  test('no GPU shows the empty GPU card', async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await mockHw(page, [])
    await page.goto(`${testInfra.baseURL}/settings/hardware`)
    await expect(byTestId(page, 'hardware-gpu-none-card')).toBeVisible({ timeout: 30000 })
  })

  test('multiple GPUs render one card per device', async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const gpus = [
      { device_id: 'gpu-0', name: 'NVIDIA A100', vendor: 'NVIDIA', memory: 40 * 1024 * 1024 * 1024, compute_capabilities: {} },
      { device_id: 'gpu-1', name: 'NVIDIA H100', vendor: 'NVIDIA', memory: 80 * 1024 * 1024 * 1024, compute_capabilities: {} },
    ]
    await mockHw(page, gpus)
    await page.goto(`${testInfra.baseURL}/settings/hardware`)
    // One card per device, titled by the device name (dynamic mock data).
    await expect(byTestId(page, 'hardware-gpu-info-card-0')).toContainText('NVIDIA A100', { timeout: 30000 })
    await expect(byTestId(page, 'hardware-gpu-info-card-1')).toContainText('NVIDIA H100')
    await expect(byTestId(page, 'hardware-gpu-none-card')).toHaveCount(0)
  })
})
