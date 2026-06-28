import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-bd78e9797f84 — the hardware-monitoring SSE flow + real-time
// display was completely untested: Hardware.store subscribes to
// /api/hardware/usage-stream and sets `currentUsage` on each `update` event,
// which HardwareSettings renders as CPU/Memory Progress bars. We mock ONLY the
// SSE upstream (the external boundary) to emit a `connected` then an `update`
// frame, and assert the live values reach the UI.

async function mockHardware(page: Page) {
  // Static hardware info so the page renders past its loading branch.
  await page.route(/\/api\/hardware$/, async (route, req) => {
    if (req.method() !== 'GET') return route.continue()
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hardware: {
          os_name: 'Linux',
          os_version: 'test',
          cpu_brand: 'Test CPU',
          cpu_cores: 8,
          total_memory: 16 * 1024 * 1024 * 1024,
          gpus: [],
        },
      }),
    })
  })

  // The usage SSE: a `connected` handshake then one `update` carrying live
  // CPU/Memory numbers. A complete body delivers both frames then EOF.
  const frames =
    'event: connected\ndata: {"message":"Connected"}\n\n' +
    'event: update\n' +
    'data: ' +
    JSON.stringify({
      cpu: { usage_percentage: 42.5 },
      memory: {
        available_ram: 8 * 1024 * 1024 * 1024,
        used_ram: 8 * 1024 * 1024 * 1024,
        usage_percentage: 55.0,
      },
      gpu_devices: [],
      timestamp: '2026-06-28T00:00:00Z',
    }) +
    '\n\n'
  await page.route(/\/api\/hardware\/usage-stream/, async route => {
    await route.fulfill({
      status: 200,
      headers: { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' },
      body: frames,
    })
  })
}

test.describe('Hardware monitoring — SSE real-time display', () => {
  test('an SSE update renders live CPU usage', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await mockHardware(page)

    await page.goto(`${baseURL}/settings/hardware`)

    // The CPU usage card + the live percentage from the SSE `update` frame.
    await expect(page.getByText('CPU Usage')).toBeVisible({ timeout: 30000 })
    await expect(page.getByText('42.5%')).toBeVisible({ timeout: 15000 })
  })
})
