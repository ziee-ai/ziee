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
  // macOS libkrun + WSL2 fields added in sandbox cross-platform work;
  // keep mock in sync with backend `CodeSandboxResourceLimits`.
  mac_vm_vcpus: number
  mac_vm_ram_mib: number
  vm_max_concurrent_execs: number
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
    mac_vm_vcpus: 4,
    mac_vm_ram_mib: 4096,
    vm_max_concurrent_execs: 2,
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
      // Merged single-page route (was /settings/sandbox-resource-limits).
      await page.goto(`${baseURL}/settings/sandbox`)
      // Page heading is now the combined "Code Sandbox" container; the
      // limits surface lives in the "Resource limits" Card section.
      const heading = page.getByRole('heading', { name: 'Code Sandbox' })
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

  test('rejects a malformed cpu_max before submit', async ({
    page,
    testInfra,
  }) => {
    // cpu_max is a free-form text field (regex pattern validator), so
    // AntD does NOT auto-clamp the input back into range the way the
    // numeric InputNumbers do. That makes it the right field to exercise
    // the "form validator blocks PUT" path end-to-end. Numeric clamping
    // is itself part of the contract — covered separately by the unit
    // tests in resource_limits.rs:validate() and the Tier-3 422-response
    // tests in tier3_resource_limits.rs.
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    // "abc 100000" violates the `^[0-9]+ [0-9]+$` pattern.
    const cpu = page.getByLabel('cgroup cpu.max')
    await cpu.fill('abc 100000')
    await page.getByRole('button', { name: 'Save' }).click()

    // AntD Form.Item renders the validator error inline.
    await expect(
      page.getByText('shape: "<quota> <period>" (digits)').first(),
    ).toBeVisible({ timeout: 5000 })
    // And the server PUT must not have fired.
    expect(state.lastPatch).toBeNull()
  })

  // audit id all-8a063de84792 — the edit test only changes memory.max + pids.max;
  // the rest of the form (cpu.max, wall-clock timeout, rlimit --nofile,
  // idle-evict) was never exercised through the UI. This edits those remaining
  // fields, Saves, and asserts they all reach the PUT patch.
  test('edits the remaining limit fields and Save sends them all', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: { current: Row; lastPatch: Partial<Row> | null } = {
      current: defaults(),
      lastPatch: null,
    }
  test('Reset reverts an edited field and clears the dirty state', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    await page.getByLabel('cgroup cpu.max').fill('50000 100000')
    await page.getByLabel('Wall-clock per-exec timeout').fill('300')
    await page.getByLabel('rlimit --nofile').fill('2048')
    await page.getByLabel('Idle-evict timeout').fill('600')
    const mem = page.getByLabel('memory.max')
    await expect(mem).toHaveValue('512')

    // Reset is disabled until the form is dirty.
    const reset = page.getByRole('button', { name: 'Reset' })
    await expect(reset).toBeDisabled()

    // Edit a field → the form becomes dirty and Reset enables.
    await mem.fill('256')
    await expect(reset).toBeEnabled()

    // Reset → the field reverts to the loaded value, Reset disables again,
    // and no PUT was issued.
    await reset.click()
    await expect(mem).toHaveValue('512')
    await expect(reset).toBeDisabled()
    expect(state.lastPatch).toBeNull()
  })

  test('a failed save surfaces an error message', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    // Override the PUT to fail (registered after mockLimits → takes precedence).
    await page.route(/\/api\/code-sandbox\/resource-limits$/, async (route, req) => {
      if (req.method() === 'PUT') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'boom' } }),
        })
      }
      return route.fallback()
    })
    await gotoResourceLimits(page, baseURL)

    // Make a valid edit, then Save → the 500 surfaces an error message.
    await page.getByLabel('memory.max').fill('256')
    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.locator('.ant-message-error').first()).toBeVisible({
      timeout: 10000,
    })
  })

  // The earlier edit→Save test only touches the core cgroup fields; the VM-tuning
  // half of the form (SandboxResourceLimitsSection.tsx:333-424 — idle-evict,
  // per-VM concurrency, mac vCPUs/RAM) was never exercised through the UI.
  test('edits the VM-tuning fields and the Save PATCH carries them', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const state = { current: defaults(), lastPatch: null as Partial<Row> | null }
    await loginAsAdmin(page, baseURL)
    await mockLimits(page, state)
    await gotoResourceLimits(page, baseURL)

    await page.getByLabel('Idle-evict timeout').fill('1200')
    await page.getByLabel('Concurrent execs per VM / distro').fill('4')
    await page.getByLabel('vCPUs').fill('8')
    await page.getByLabel('RAM ceiling').fill('8192')

    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Resource limits saved')).toBeVisible({ timeout: 5000 })

    expect(state.lastPatch).not.toBeNull()
    expect(state.lastPatch?.cpu_max).toBe('50000 100000')
    expect(state.lastPatch?.timeout_secs).toBe(300)
    expect(state.lastPatch?.nofile_max).toBe(2048)
    expect(state.lastPatch?.vm_idle_evict_secs).toBe(600)
    expect(state.lastPatch?.vm_idle_evict_secs).toBe(1200)
    expect(state.lastPatch?.vm_max_concurrent_execs).toBe(4)
    expect(state.lastPatch?.mac_vm_vcpus).toBe(8)
    expect(state.lastPatch?.mac_vm_ram_mib).toBe(8192)

    // Reload → fresh GET → the VM-tuning values persisted in the form.
    await gotoResourceLimits(page, baseURL)
    await expect(page.getByLabel('Idle-evict timeout')).toHaveValue('1200')
    await expect(page.getByLabel('vCPUs')).toHaveValue('8')
  })
})
