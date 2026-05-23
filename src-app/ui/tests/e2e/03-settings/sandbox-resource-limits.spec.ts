import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// ---------------------------------------------------------------------------
// Route mocks. Resource-limits is a singleton row backed by a fixed Postgres
// table; the wire shape is fully specified by the OpenAPI client. We mock
// both the GET (initial load) and the PUT (save), tracking the current row
// in test-scoped state so the post-save reload sees the updated values.
// ---------------------------------------------------------------------------

type Row = {
  memory_max_bytes: number
  memory_swap_max_bytes: number
  pids_max: number
  cpu_max: string
  address_space_bytes: number
  fsize_bytes: number
  nproc_max: number
  nofile_max: number
  cpu_secs_max: number
  timeout_secs: number
  vm_idle_evict_secs: number
  created_at: string
  updated_at: string
}

function defaults(): Row {
  const now = new Date().toISOString()
  return {
    memory_max_bytes: 512 * 1024 * 1024,
    memory_swap_max_bytes: 0,
    pids_max: 256,
    cpu_max: '100000 100000',
    address_space_bytes: 4 * 1024 * 1024 * 1024,
    fsize_bytes: 256 * 1024 * 1024,
    nproc_max: 256,
    nofile_max: 1024,
    cpu_secs_max: 1240,
    timeout_secs: 620,
    vm_idle_evict_secs: 900,
    created_at: now,
    updated_at: now,
  }
}

async function mockLimits(
  page: Page,
  state: { current: Row; lastPatch: Partial<Row> | null },
) {
  await page.route(
    /\/api\/code-sandbox\/resource-limits$/,
    async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(state.current),
        })
      }
      if (req.method() === 'PUT') {
        const patch = JSON.parse(req.postData() ?? '{}') as Partial<Row>
        state.lastPatch = patch
        // PATCH semantics — only the supplied keys override.
        state.current = {
          ...state.current,
          ...patch,
          updated_at: new Date().toISOString(),
        }
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(state.current),
        })
      }
      return route.continue()
    },
  )
}

async function gotoResourceLimits(page: Page, baseURL: string) {
  // Same retry-on-Vite-504 pattern as sandbox-environments-admin.
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await page.goto(`${baseURL}/settings/sandbox-resource-limits`)
      const heading = page.getByRole('heading', {
        name: 'Sandbox resource limits',
      })
      await expect(heading).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

// ---------------------------------------------------------------------------

test.describe('Sandbox resource limits admin settings', () => {
  test.describe.configure({ retries: 2 })

  test('loads the defaults and renders them in the form', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    // Memory in MiB (form converts bytes ↔ MiB on read/write).
    await expect(page.getByLabel('memory.max')).toHaveValue('512')
    await expect(page.getByLabel('cgroup pids.max')).toHaveValue('256')
    await expect(page.getByLabel('cgroup cpu.max')).toHaveValue('100000 100000')
    await expect(
      page.getByLabel('Wall-clock per-exec timeout'),
    ).toHaveValue('620')
  })

  test('edit → Save persists; reload still shows the new value', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    // Halve memory: 512 MiB → 256 MiB.
    const mem = page.getByLabel('memory.max')
    await mem.fill('256')

    // Lower pids_max to 128.
    const pids = page.getByLabel('cgroup pids.max')
    await pids.fill('128')

    // Save.
    await page.getByRole('button', { name: 'Save' }).click()
    // The handler returns the updated row; the success toast then fires.
    await expect(page.getByText('Resource limits saved')).toBeVisible({
      timeout: 5000,
    })

    // Confirm the wire-level PATCH carried bytes (256 MiB → 268435456) +
    // the new pids cap.
    expect(state.lastPatch).not.toBeNull()
    expect(state.lastPatch?.memory_max_bytes).toBe(256 * 1024 * 1024)
    expect(state.lastPatch?.pids_max).toBe(128)

    // Reload → fresh GET → form still shows the patched values.
    await gotoResourceLimits(page, baseURL)
    await expect(page.getByLabel('memory.max')).toHaveValue('256')
    await expect(page.getByLabel('cgroup pids.max')).toHaveValue('128')
  })

  test('rejects an out-of-range memory value before submit', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    // 8 MiB is below the 16 MiB floor.
    await page.getByLabel('memory.max').fill('8')
    await page.getByRole('button', { name: 'Save' }).click()

    // AntD form validators render the message inline. The server PUT
    // should NOT have fired.
    await expect(page.getByText('must be ≥ 16 MiB').first()).toBeVisible({
      timeout: 5000,
    })
    expect(state.lastPatch).toBeNull()
  })
})
